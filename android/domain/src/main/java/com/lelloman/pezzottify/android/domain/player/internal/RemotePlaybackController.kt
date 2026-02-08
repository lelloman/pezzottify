package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Implements ControlsAndStatePlayer by deriving state from a remote device's
 * playback state (via PlaybackSessionHandler) and sending commands via WebSocket.
 */
@OptIn(DelicateCoroutinesApi::class)
@Singleton
class RemotePlaybackController internal constructor(
    private val playbackSessionHandler: PlaybackSessionHandler,
    private val playbackModeManager: PlaybackModeManager,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : ControlsAndStatePlayer {

    @Inject
    constructor(
        playbackSessionHandler: PlaybackSessionHandler,
        playbackModeManager: PlaybackModeManager,
        loggerFactory: LoggerFactory,
    ) : this(playbackSessionHandler, playbackModeManager, GlobalScope, loggerFactory)

    private val logger = loggerFactory.getLogger(RemotePlaybackController::class)

    private val _isActive = MutableStateFlow(false)
    override val isActive: StateFlow<Boolean> = _isActive.asStateFlow()

    private val _isPlaying = MutableStateFlow(false)
    override val isPlaying: StateFlow<Boolean> = _isPlaying.asStateFlow()

    private val _volumeState = MutableStateFlow(VolumeState(1f, false))
    override val volumeState: StateFlow<VolumeState> = _volumeState.asStateFlow()

    private val _currentTrackIndex = MutableStateFlow<Int?>(null)
    override val currentTrackIndex: StateFlow<Int?> = _currentTrackIndex.asStateFlow()

    private val _currentTrackPercent = MutableStateFlow<Float?>(null)
    override val currentTrackPercent: StateFlow<Float?> = _currentTrackPercent.asStateFlow()

    private val _currentTrackProgressSec = MutableStateFlow<Int?>(null)
    override val currentTrackProgressSec: StateFlow<Int?> = _currentTrackProgressSec.asStateFlow()

    private val _currentTrackDurationSeconds = MutableStateFlow<Int?>(null)
    override val currentTrackDurationSeconds: StateFlow<Int?> = _currentTrackDurationSeconds.asStateFlow()

    private val _seekEvents = MutableSharedFlow<ControlsAndStatePlayer.SeekEvent>()
    override val seekEvents: SharedFlow<ControlsAndStatePlayer.SeekEvent> = _seekEvents.asSharedFlow()

    private val _playerError = MutableStateFlow<ControlsAndStatePlayer.PlayerError?>(null)
    override val playerError: StateFlow<ControlsAndStatePlayer.PlayerError?> = _playerError.asStateFlow()

    private val _shuffleEnabled = MutableStateFlow(false)
    override val shuffleEnabled: StateFlow<Boolean> = _shuffleEnabled.asStateFlow()

    private val _repeatMode = MutableStateFlow(RepeatMode.OFF)
    override val repeatMode: StateFlow<RepeatMode> = _repeatMode.asStateFlow()

    private var interpolationJob: Job? = null
    private var stateCollectionJob: Job? = null

    fun initialize() {
        // Observe mode changes to start/stop tracking
        scope.launch {
            playbackModeManager.mode.collect { mode ->
                when (mode) {
                    is PlaybackMode.Remote -> startTracking(mode.deviceId)
                    is PlaybackMode.Local -> stopTracking()
                }
            }
        }
    }

    private fun startTracking(deviceId: Int) {
        stopTracking()
        _isActive.value = true
        logger.info("Starting remote tracking for device $deviceId")

        stateCollectionJob = scope.launch {
            playbackSessionHandler.otherDeviceStates
                .map { it[deviceId] }
                .collect { remoteState ->
                    if (remoteState != null) {
                        updateFromRemoteState(remoteState)
                    } else {
                        // Remote device disconnected or stopped
                        logger.info("Remote device $deviceId state became null, exiting remote mode")
                        playbackModeManager.exitRemoteMode()
                    }
                }
        }

        // Start progress interpolation
        interpolationJob = scope.launch {
            while (true) {
                delay(INTERPOLATION_TICK_MS)
                interpolateProgress()
            }
        }
    }

    private fun stopTracking() {
        stateCollectionJob?.cancel()
        stateCollectionJob = null
        interpolationJob?.cancel()
        interpolationJob = null
        _isActive.value = false
        _isPlaying.value = false
        _currentTrackIndex.value = null
        _currentTrackPercent.value = null
        _currentTrackProgressSec.value = null
        _currentTrackDurationSeconds.value = null
        _playerError.value = null
    }

    private var lastRemoteState: RemotePlaybackState? = null
    private var lastStateReceivedAt: Long = 0L

    private fun updateFromRemoteState(state: RemotePlaybackState) {
        lastRemoteState = state
        lastStateReceivedAt = System.currentTimeMillis()

        _isPlaying.value = state.isPlaying
        _volumeState.value = VolumeState(state.volume, state.muted)
        _shuffleEnabled.value = state.shuffle
        _repeatMode.value = when (state.repeat) {
            "all" -> RepeatMode.ALL
            "one" -> RepeatMode.ONE
            else -> RepeatMode.OFF
        }
        _currentTrackIndex.value = state.queuePosition

        val durationMs = state.currentTrack?.durationMs ?: 0L
        val durationSec = (durationMs / 1000).toInt()
        _currentTrackDurationSeconds.value = if (durationSec > 0) durationSec else null

        updateProgress(state.position, durationMs)
    }

    private fun interpolateProgress() {
        val state = lastRemoteState ?: return
        if (!state.isPlaying) return

        val elapsed = (System.currentTimeMillis() - lastStateReceivedAt) / 1000.0
        val interpolatedPosition = state.position + elapsed
        val durationMs = state.currentTrack?.durationMs ?: 0L
        updateProgress(interpolatedPosition.coerceAtMost(durationMs / 1000.0), durationMs)
    }

    private fun updateProgress(positionSec: Double, durationMs: Long) {
        _currentTrackProgressSec.value = positionSec.toInt()
        if (durationMs > 0) {
            _currentTrackPercent.value = ((positionSec * 1000.0 / durationMs) * 100f).toFloat().coerceIn(0f, 100f)
        } else {
            _currentTrackPercent.value = null
        }
    }

    private val targetDeviceId: Int?
        get() = (playbackModeManager.mode.value as? PlaybackMode.Remote)?.deviceId

    private fun sendCommand(command: String, payload: Map<String, Any?> = emptyMap()) {
        val deviceId = targetDeviceId ?: return
        playbackSessionHandler.sendCommand(command, payload, deviceId)
    }

    // ControlsAndStatePlayer control methods - send commands to remote device

    override fun togglePlayPause() {
        sendCommand(if (_isPlaying.value) "pause" else "play")
    }

    override fun setIsPlaying(isPlaying: Boolean) {
        sendCommand(if (isPlaying) "play" else "pause")
    }

    override fun seekToPercentage(percentage: Float) {
        val durationSec = _currentTrackDurationSeconds.value ?: return
        val positionSec = (percentage / 100f) * durationSec
        sendCommand("seek", mapOf("position" to positionSec.toDouble()))
    }

    override fun forward10Sec() {
        val currentSec = _currentTrackProgressSec.value ?: return
        val durationSec = _currentTrackDurationSeconds.value ?: return
        val newPosition = (currentSec + 10).coerceAtMost(durationSec).toDouble()
        sendCommand("seek", mapOf("position" to newPosition))
    }

    override fun rewind10Sec() {
        val currentSec = _currentTrackProgressSec.value ?: return
        val newPosition = (currentSec - 10).coerceAtLeast(0).toDouble()
        sendCommand("seek", mapOf("position" to newPosition))
    }

    override fun stop() {
        sendCommand("pause")
    }

    override fun setVolume(volume: Float) {
        sendCommand("setVolume", mapOf("volume" to volume.toDouble()))
    }

    override fun setMuted(isMuted: Boolean) {
        sendCommand("setMuted", mapOf("muted" to isMuted))
    }

    override fun loadTrackIndex(index: Int) {
        sendCommand("skipToTrack", mapOf("index" to index))
    }

    override fun skipToNextTrack() {
        sendCommand("next")
    }

    override fun skipToPreviousTrack() {
        sendCommand("prev")
    }

    override fun toggleShuffle() {
        sendCommand("setShuffle", mapOf("enabled" to !_shuffleEnabled.value))
    }

    override fun cycleRepeatMode() {
        val nextMode = when (_repeatMode.value) {
            RepeatMode.OFF -> "all"
            RepeatMode.ALL -> "one"
            RepeatMode.ONE -> "off"
        }
        sendCommand("setRepeat", mapOf("mode" to nextMode))
    }

    override fun retry() {
        // Remote retry - just try to play
        sendCommand("play")
    }

    companion object {
        private const val INTERPOLATION_TICK_MS = 200L
    }
}
