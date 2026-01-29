package com.lelloman.pezzottify.android.domain.playback

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class RemotePlaybackManagerImpl @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val player: PezzottifyPlayer,
    private val platformPlayer: PlatformPlayer,
    private val metadataProvider: PlaybackMetadataProvider,
    private val configStore: ConfigStore,
    loggerFactory: LoggerFactory,
    @IoDispatcher dispatcher: CoroutineDispatcher,
) : RemotePlaybackManager {

    private val logger = loggerFactory.getLogger(RemotePlaybackManagerImpl::class)
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    // State
    private val _deviceId = MutableStateFlow<Long?>(null)
    override val deviceId: StateFlow<Long?> = _deviceId.asStateFlow()

    private val _devices = MutableStateFlow<List<PlaybackDevice>>(emptyList())
    override val devices: StateFlow<List<PlaybackDevice>> = _devices.asStateFlow()

    private val _selectedOutputDevice = MutableStateFlow<Long?>(null)
    override val selectedOutputDevice: StateFlow<Long?> = _selectedOutputDevice.asStateFlow()

    private val _sessionExists = MutableStateFlow(false)
    override val sessionExists: StateFlow<Boolean> = _sessionExists.asStateFlow()

    private val _remoteState = MutableStateFlow<PlaybackState?>(null)
    override val remoteState: StateFlow<PlaybackState?> = _remoteState.asStateFlow()

    private val _remoteQueue = MutableStateFlow<List<QueueItem>>(emptyList())
    override val remoteQueue: StateFlow<List<QueueItem>> = _remoteQueue.asStateFlow()

    private val _interpolatedPosition = MutableStateFlow(0f)
    override val interpolatedPosition: StateFlow<Float> = _interpolatedPosition.asStateFlow()

    private val _isLocalOutput = MutableStateFlow(true)
    override val isLocalOutput: StateFlow<Boolean> = _isLocalOutput.asStateFlow()

    private val _isAudioDevice = MutableStateFlow(false)
    override val isAudioDevice: StateFlow<Boolean> = _isAudioDevice.asStateFlow()

    // Internal state
    private var remoteQueueVersion = 0
    private var queueVersion = 0
    private var broadcastJob: Job? = null
    private var interpolationJob: Job? = null
    private var pendingTransferId: String? = null

    // ============================================
    // Message handling (called by PlaybackWebSocketHandler)
    // ============================================

    fun handleWelcome(payload: WelcomePayload) {
        _deviceId.value = payload.deviceId
        _devices.value = payload.devices
        _sessionExists.value = payload.session.exists

        if (payload.session.exists && payload.session.state != null) {
            _remoteState.value = payload.session.state
            _remoteQueue.value = payload.session.queue ?: emptyList()
            remoteQueueVersion = payload.session.state.queueVersion

            // Determine output device based on current audio device
            val currentAudioDevice = payload.devices.find { it.isAudioDevice }
            if (currentAudioDevice != null) {
                if (currentAudioDevice.id == payload.deviceId) {
                    // We are the audio device
                    _selectedOutputDevice.value = null
                    _isLocalOutput.value = true
                    _isAudioDevice.value = true
                    startBroadcasting()
                } else {
                    // Another device is the audio device
                    _selectedOutputDevice.value = currentAudioDevice.id
                    _isLocalOutput.value = false
                    _isAudioDevice.value = false
                    startInterpolation()
                }
            }
        }

        logger.info("Welcome received, device ID: ${payload.deviceId}")
    }

    fun handleRemoteState(state: PlaybackState) {
        if (_isAudioDevice.value) return // Ignore if we're the audio device

        _remoteState.value = state

        // Check queue version
        if (state.queueVersion > remoteQueueVersion) {
            requestQueueSync()
        }
    }

    fun handleQueueSync(payload: QueuePayload) {
        _remoteQueue.value = payload.queue
        remoteQueueVersion = payload.queueVersion
    }

    fun handleSessionEnded(reason: String?) {
        logger.info("Session ended: $reason")
        _sessionExists.value = false
        _remoteState.value = null
        _remoteQueue.value = emptyList()
        _selectedOutputDevice.value = null
        _isLocalOutput.value = true
        _isAudioDevice.value = false
        stopInterpolation()
        stopBroadcasting()
    }

    fun handleDeviceListChanged(payload: DeviceListChangedPayload) {
        _devices.value = payload.devices

        // Update selection if audio device changed
        val currentAudioDevice = payload.devices.find { it.isAudioDevice }
        if (currentAudioDevice != null) {
            if (currentAudioDevice.id == _deviceId.value) {
                _selectedOutputDevice.value = null
                _isLocalOutput.value = true
            } else if (_selectedOutputDevice.value != currentAudioDevice.id) {
                _selectedOutputDevice.value = currentAudioDevice.id
                _isLocalOutput.value = false
            }
        }

        logger.debug("Device list changed: ${payload.change.type} ${payload.change.deviceId}")
    }

    fun handleCommand(payload: CommandPayload) {
        if (!_isAudioDevice.value) return

        logger.debug("Received command: ${payload.command}")

        when (payload.command) {
            "play" -> player.setIsPlaying(true)
            "pause" -> player.setIsPlaying(false)
            "seek" -> payload.payload?.position?.let { pos ->
                val duration = player.currentTrackDurationSeconds.value?.toFloat() ?: 0f
                if (duration > 0) {
                    player.seekToPercentage(pos / duration)
                }
            }
            "next" -> player.skipToNextTrack()
            "prev" -> player.skipToPreviousTrack()
            "setVolume" -> payload.payload?.volume?.let { player.setVolume(it) }
            "setMuted" -> payload.payload?.muted?.let { player.setMuted(it) }
            "becomeAudioDevice" -> {
                payload.payload?.transferId?.let { transferId ->
                    handleBecomeAudioDeviceRequest(transferId)
                }
            }
            else -> logger.warn("Unknown command: ${payload.command}")
        }
    }

    fun handlePrepareTransfer(payload: PrepareTransferPayload) {
        // We're the current audio device, prepare to transfer
        player.setIsPlaying(false) // Pause before transfer

        val state = buildCurrentState()
        val queue = buildCurrentQueue()

        webSocketManager.send(
            "playback.transfer_ready",
            TransferReadyPayload(
                transferId = payload.transferId,
                state = state,
                queue = queue,
            ),
        )

        pendingTransferId = payload.transferId
        logger.info("Preparing transfer to ${payload.targetDeviceName}")
    }

    fun handleBecomeAudioDevice(payload: BecomeAudioDevicePayload) {
        // We're becoming the new audio device
        scope.launch {
            // Apply received playlist state to player
            applyTransferredState(payload.state, payload.queue)

            // Send transfer complete after applying state
            webSocketManager.send(
                "playback.transfer_complete",
                TransferCompletePayload(transferId = payload.transferId),
            )

            _isAudioDevice.value = true
            _isLocalOutput.value = true
            _selectedOutputDevice.value = null
            _sessionExists.value = true
            pendingTransferId = null
            startBroadcasting()
            stopInterpolation()

            // Start playback if it was playing
            if (payload.state.isPlaying) {
                player.setIsPlaying(true)
            }

            logger.info("Became audio device via transfer with ${payload.queue.size} tracks")
        }
    }

    private suspend fun applyTransferredState(state: PlaybackState, queue: List<QueueItem>) {
        if (queue.isEmpty()) {
            logger.warn("Received empty queue during transfer")
            return
        }

        val trackIds = queue.map { it.id }
        val baseUrl = configStore.baseUrl.value
        val urls = trackIds.map { "$baseUrl/v1/content/stream/$it" }

        withContext(Dispatchers.Main) {
            // Load the playlist into the platform player
            platformPlayer.loadPlaylist(urls)

            // Seek to the correct track position
            val queuePosition = state.queuePosition.coerceIn(0, trackIds.size - 1)
            if (queuePosition > 0) {
                platformPlayer.loadTrackIndex(queuePosition)
            }

            // Seek to the playback position within the track
            if (state.position > 0) {
                val duration = state.currentTrack?.duration ?: 0f
                if (duration > 0) {
                    val percentage = (state.position / duration).coerceIn(0f, 1f)
                    platformPlayer.seekToPercentage(percentage)
                }
            }

            // Apply volume and muted state
            platformPlayer.setVolume(state.volume)
            platformPlayer.setMuted(state.muted)
        }

        // Update queue version to match received state
        queueVersion = state.queueVersion

        logger.info("Applied transferred state: ${trackIds.size} tracks, position ${state.queuePosition}, seek ${state.position}s")
    }

    fun handleTransferComplete() {
        // Old audio device - transfer succeeded
        player.stop()

        // Update selected output to new audio device
        val newAudioDevice = _devices.value.find { it.isAudioDevice }
        if (newAudioDevice != null) {
            _selectedOutputDevice.value = newAudioDevice.id
            _isLocalOutput.value = false
        }

        _isAudioDevice.value = false
        pendingTransferId = null
        stopBroadcasting()
        startInterpolation()

        logger.info("Transfer complete, stopped local playback")
    }

    fun handleTransferAborted(payload: TransferAbortedPayload) {
        logger.info("Transfer aborted: ${payload.reason}")
        pendingTransferId = null

        // If we were source, resume playback
        if (_isAudioDevice.value) {
            player.setIsPlaying(true)
        }
    }

    // ============================================
    // Output device selection
    // ============================================

    override fun selectOutputDevice(deviceId: Long?) {
        val myDeviceId = _deviceId.value ?: return

        if (deviceId == null || deviceId == myDeviceId) {
            // Select this device as output
            if (!_isAudioDevice.value && _sessionExists.value) {
                // Need to transfer playback to this device
                requestBecomeAudioDevice()
            }
            _selectedOutputDevice.value = null
            _isLocalOutput.value = true
        } else {
            // Select a remote device as output
            _selectedOutputDevice.value = deviceId
            _isLocalOutput.value = false

            val currentAudioDevice = _devices.value.find { it.isAudioDevice }
            if (currentAudioDevice?.id != deviceId) {
                // Could implement transfer to remote device here
                logger.info("Selected remote output: $deviceId")
            }

            startInterpolation()
        }
    }

    // ============================================
    // Unified playback commands
    // ============================================

    override fun play() {
        if (_isLocalOutput.value) {
            player.setIsPlaying(true)
        } else {
            sendCommand("play")
        }
    }

    override fun pause() {
        if (_isLocalOutput.value) {
            player.setIsPlaying(false)
        } else {
            sendCommand("pause")
        }
    }

    override fun playPause() {
        if (_isLocalOutput.value) {
            player.togglePlayPause()
        } else {
            val currentlyPlaying = _remoteState.value?.isPlaying ?: false
            sendCommand(if (currentlyPlaying) "pause" else "play")
        }
    }

    override fun seek(positionSec: Float) {
        if (_isLocalOutput.value) {
            val duration = player.currentTrackDurationSeconds.value?.toFloat() ?: 0f
            if (duration > 0) {
                player.seekToPercentage(positionSec / duration)
            }
        } else {
            sendCommand("seek", CommandData(position = positionSec))
        }
    }

    override fun seekToPercentage(percent: Float) {
        if (_isLocalOutput.value) {
            player.seekToPercentage(percent)
        } else {
            val duration = _remoteState.value?.currentTrack?.duration ?: 0f
            if (duration > 0) {
                sendCommand("seek", CommandData(position = percent * duration))
            }
        }
    }

    override fun skipNext() {
        if (_isLocalOutput.value) {
            player.skipToNextTrack()
        } else {
            sendCommand("next")
        }
    }

    override fun skipPrevious() {
        if (_isLocalOutput.value) {
            player.skipToPreviousTrack()
        } else {
            sendCommand("prev")
        }
    }

    override fun forward10Sec() {
        if (_isLocalOutput.value) {
            player.forward10Sec()
        } else {
            val currentPos = _interpolatedPosition.value
            sendCommand("seek", CommandData(position = currentPos + 10))
        }
    }

    override fun rewind10Sec() {
        if (_isLocalOutput.value) {
            player.rewind10Sec()
        } else {
            val currentPos = _interpolatedPosition.value
            sendCommand("seek", CommandData(position = maxOf(0f, currentPos - 10)))
        }
    }

    override fun setVolume(volume: Float) {
        if (_isLocalOutput.value) {
            player.setVolume(volume)
        } else {
            sendCommand("setVolume", CommandData(volume = volume))
        }
    }

    override fun setMuted(muted: Boolean) {
        if (_isLocalOutput.value) {
            player.setMuted(muted)
        } else {
            sendCommand("setMuted", CommandData(muted = muted))
        }
    }

    override fun stop() {
        if (_isLocalOutput.value) {
            player.stop()
        }
        // Remote stop doesn't make sense
    }

    // ============================================
    // Audio device management
    // ============================================

    override fun registerAsAudioDevice() {
        webSocketManager.send("playback.register_audio_device", null)
        _isAudioDevice.value = true
        _isLocalOutput.value = true
        _selectedOutputDevice.value = null
        _sessionExists.value = true
        startBroadcasting()
        stopInterpolation()
        logger.info("Registered as audio device")
    }

    override fun unregisterAsAudioDevice() {
        webSocketManager.send("playback.unregister_audio_device", null)
        _isAudioDevice.value = false
        stopBroadcasting()
        logger.info("Unregistered as audio device")
    }

    override fun broadcastState(state: PlaybackState) {
        if (!_isAudioDevice.value) return
        webSocketManager.send("playback.state", state)
    }

    override fun broadcastQueue(queue: List<QueueItem>, version: Int) {
        if (!_isAudioDevice.value) return
        queueVersion = version
        webSocketManager.send(
            "playback.queue_update",
            QueuePayload(queue = queue, queueVersion = version),
        )
    }

    // ============================================
    // Internal helpers
    // ============================================

    private fun sendCommand(command: String, data: CommandData? = null) {
        webSocketManager.send(
            "playback.command",
            CommandPayload(command = command, payload = data),
        )
    }

    private fun requestQueueSync() {
        webSocketManager.send("playback.request_queue", null)
    }

    private fun requestBecomeAudioDevice() {
        val transferId = java.util.UUID.randomUUID().toString()
        pendingTransferId = transferId
        sendCommand("becomeAudioDevice", CommandData(transferId = transferId))
        logger.info("Requesting to become audio device")
    }

    private fun handleBecomeAudioDeviceRequest(transferId: String) {
        // Another device wants to become audio device
        // This triggers prepare_transfer from server
        logger.debug("Received becomeAudioDevice request: $transferId")
    }

    private fun startBroadcasting() {
        if (broadcastJob != null) return

        broadcastJob = scope.launch {
            while (isActive) {
                delay(BROADCAST_INTERVAL_MS)
                if (_isAudioDevice.value && player.isPlaying.value) {
                    val state = buildCurrentState()
                    broadcastState(state)
                }
            }
        }

        // Broadcast immediately
        scope.launch {
            val state = buildCurrentState()
            broadcastState(state)
        }
    }

    private fun stopBroadcasting() {
        broadcastJob?.cancel()
        broadcastJob = null
    }

    private fun startInterpolation() {
        if (interpolationJob != null) return

        interpolationJob = scope.launch {
            while (isActive) {
                delay(16) // ~60fps
                val state = _remoteState.value ?: continue
                if (state.isPlaying) {
                    val elapsed = (System.currentTimeMillis() - state.timestamp) / 1000f
                    _interpolatedPosition.value = state.position + elapsed
                } else {
                    _interpolatedPosition.value = state.position
                }
            }
        }
    }

    private fun stopInterpolation() {
        interpolationJob?.cancel()
        interpolationJob = null
    }

    private fun buildCurrentState(): PlaybackState {
        val queueState = metadataProvider.queueState.value
        val currentTrackMetadata = queueState?.currentTrack

        val currentTrack = currentTrackMetadata?.let { metadata ->
            PlaybackTrack(
                id = metadata.trackId,
                title = metadata.trackName,
                artistId = metadata.primaryArtistId,
                artistName = metadata.artistNames.firstOrNull() ?: "Unknown Artist",
                albumId = metadata.albumId,
                albumTitle = metadata.albumName,
                duration = metadata.durationSeconds.toFloat(),
                trackNumber = null, // Not available in metadata
                imageId = metadata.imageId,
            )
        }

        return PlaybackState(
            currentTrack = currentTrack,
            queuePosition = player.currentTrackIndex.value ?: 0,
            queueVersion = queueVersion,
            position = player.currentTrackProgressSec.value?.toFloat() ?: 0f,
            isPlaying = player.isPlaying.value,
            volume = player.volumeState.value.volume,
            muted = player.volumeState.value.isMuted,
            shuffle = player.shuffleEnabled.value,
            repeat = player.repeatMode.value.name.lowercase(),
            timestamp = System.currentTimeMillis(),
        )
    }

    private fun buildCurrentQueue(): List<QueueItem> {
        val playlist = player.playbackPlaylist.value ?: return emptyList()
        val now = System.currentTimeMillis()

        return playlist.tracksIds.map { trackId ->
            QueueItem(
                id = trackId,
                addedAt = now, // We don't track when tracks were added, use current time
            )
        }
    }

    companion object {
        private const val BROADCAST_INTERVAL_MS = 5000L
    }
}
