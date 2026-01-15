package com.lelloman.pezzottify.android.player

import android.content.ComponentName
import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer.SeekEvent
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.VolumeState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

@OptIn(DelicateCoroutinesApi::class)
@UnstableApi
internal class ExoPlatformPlayer(
    private val context: Context,
    playerServiceEventsEmitter: PlayerServiceEventsEmitter,
    loggerFactory: LoggerFactory,
    private val coroutineScope: CoroutineScope = GlobalScope,
) : PlatformPlayer {

    private val logger: Logger by loggerFactory

    private var mediaController: MediaController? = null

    private var sessionToken: SessionToken? = null

    private enum class MediaControllerState {
        DISCONNECTED,
        CONNECTING,
        CONNECTED,
    }

    private val mutableControllerState = MutableStateFlow(MediaControllerState.DISCONNECTED)

    private var progressPollingJob: Job? = null

    private var pendingTrackIndex: Int? = null

    private val mutableIsActive = MutableStateFlow(false)
    override val isActive = mutableIsActive.asStateFlow()

    private val mutableIsPlaying = MutableStateFlow(false)
    override val isPlaying = mutableIsPlaying.asStateFlow()

    private val mutableCurrentTrackProgressSec: MutableStateFlow<Int?> = MutableStateFlow(null)
    override val currentTrackProgressSec = mutableCurrentTrackProgressSec.asStateFlow()

    private val mutableCurrentTrackPercent: MutableStateFlow<Float?> = MutableStateFlow(null)
    override val currentTrackPercent = mutableCurrentTrackPercent.asStateFlow()

    private val mutableCurrentTrackIndex: MutableStateFlow<Int?> = MutableStateFlow(null)
    override val currentTrackIndex: StateFlow<Int?> = mutableCurrentTrackIndex.asStateFlow()

    private val mutableVolumeState = MutableStateFlow(VolumeState(0.5f, false))
    override val volumeState: StateFlow<VolumeState> = mutableVolumeState.asStateFlow()

    private val mutableShuffleEnabled = MutableStateFlow(false)
    override val shuffleEnabled: StateFlow<Boolean> = mutableShuffleEnabled.asStateFlow()

    private val mutableRepeatMode = MutableStateFlow(RepeatMode.OFF)
    override val repeatMode: StateFlow<RepeatMode> = mutableRepeatMode.asStateFlow()

    private val mutableCurrentTrackDurationSeconds = MutableStateFlow<Int?>(null)
    override val currentTrackDurationSeconds: StateFlow<Int?> = mutableCurrentTrackDurationSeconds.asStateFlow()

    private val mutableSeekEvents = MutableSharedFlow<SeekEvent>()
    override val seekEvents: SharedFlow<SeekEvent> = mutableSeekEvents.asSharedFlow()

    private val mutablePlayerError = MutableStateFlow<ControlsAndStatePlayer.PlayerError?>(null)
    override val playerError: StateFlow<ControlsAndStatePlayer.PlayerError?> = mutablePlayerError.asStateFlow()

    private var lastKnownPositionMs: Long = 0L

    private var autoRetryJob: Job? = null

    private val playerListener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            super.onEvents(player, events)
            for (i in 0 until events.size()) {
                when (events.get(i)) {
                    Player.EVENT_PLAY_WHEN_READY_CHANGED -> {
                        mutableIsPlaying.value = player.playWhenReady
                        updateProgressPolling(player.playWhenReady)
                        updateControllerState()
                    }

                    Player.EVENT_POSITION_DISCONTINUITY -> {
                        mutableCurrentTrackIndex.value = player.currentMediaItemIndex
                    }
                }
            }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            super.onPlaybackStateChanged(playbackState)
            val playbackStateText = when (playbackState) {
                Player.STATE_IDLE -> "STATE_IDLE"
                Player.STATE_BUFFERING -> "STATE_BUFFERING"
                Player.STATE_READY -> "STATE_READY"
                Player.STATE_ENDED -> "STATE_ENDED"
                else -> "$playbackState"
            }
            logger.debug("onPlaybackStateChanged: $playbackStateText")
            updateControllerState()

            // Clear error when entering READY state (after recovery or retry)
            if (playbackState == Player.STATE_READY && mutablePlayerError.value != null) {
                logger.info("Playback resumed successfully - clearing error state")
                mutablePlayerError.value = null
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            super.onPlayerError(error)
            logger.error("Player error: ${error.errorCodeName} - ${error.message}", error)

            // Store last known position before error
            mediaController?.let { controller ->
                lastKnownPositionMs = controller.currentPosition
                logger.debug("Stored last known position: ${lastKnownPositionMs}ms")
            }

            // Classify error as transient (recoverable) or permanent
            val isRecoverable = isTransientError(error)
            val errorMessage = when (error.errorCode) {
                PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED -> "Network connection failed"
                PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT -> "Network connection timeout"
                PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS -> "Server error"
                PlaybackException.ERROR_CODE_DECODER_INIT_FAILED -> "Decoder initialization failed"
                PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED -> "Decoder query failed"
                else -> "Playback error: ${error.errorCodeName}"
            }

            // Extract track ID from current media item URL (format: .../stream/{trackId})
            val currentTrackId = mediaController?.currentMediaItem
                ?.localConfiguration?.uri?.lastPathSegment

            // Create error state
            val playerError = ControlsAndStatePlayer.PlayerError(
                trackId = currentTrackId,
                message = errorMessage,
                errorCode = error.errorCodeName,
                isRecoverable = isRecoverable,
                positionMs = lastKnownPositionMs.takeIf { it > 0 }
            )

            mutablePlayerError.value = playerError

            // Cancel any pending auto-retry from a previous error
            autoRetryJob?.cancel()

            // Handle recovery based on error type
            if (isRecoverable) {
                logger.info("Transient error detected - will auto-retry")
                autoRetryJob = coroutineScope.launch {
                    delay(2500) // Wait 2.5 seconds before retry
                    if (mutablePlayerError.value?.isRecoverable == true) {
                        logger.info("Auto-retrying playback after transient error")
                        retryInternal()
                    }
                }
            } else {
                logger.info("Permanent error detected - skipping to next track")
                // After a PlaybackException, ExoPlayer transitions to STATE_IDLE.
                // We must call prepare() before seeking to next track.
                mediaController?.let { controller ->
                    controller.prepare()
                    controller.seekToNext()
                }
            }
        }
    }

    private fun updateProgressPolling(isPlaying: Boolean) {
        if (isPlaying) {
            startProgressPolling()
        } else {
            stopProgressPolling()
        }
    }

    private fun startProgressPolling() {
        if (progressPollingJob?.isActive == true) return
        progressPollingJob = coroutineScope.launch(Dispatchers.Main) {
            while (isActive) {
                updateProgress()
                delay(500)
            }
        }
    }

    private fun stopProgressPolling() {
        progressPollingJob?.cancel()
        progressPollingJob = null
    }

    private fun updateProgress() {
        val controller = mediaController ?: return
        val duration = controller.duration
        val position = controller.currentPosition
        if (duration > 0) {
            val percent = (position.toFloat() / duration.toFloat()) * 100f
            mutableCurrentTrackPercent.value = percent
            mutableCurrentTrackProgressSec.value = (position / 1000).toInt()
            mutableCurrentTrackDurationSeconds.value = (duration / 1000).toInt()
        }
    }

    init {
        coroutineScope.launch(Dispatchers.Main) {
            playerServiceEventsEmitter.events.collect {
                when (it) {
                    PlayerServiceEventsEmitter.Event.Shutdown -> {
                        logger.info("Received Shutdown event - clearing player state")
                        stopProgressPolling()
                        mediaController?.removeListener(playerListener)
                        mediaController?.release()
                        mediaController = null
                        sessionToken = null
                        mutableIsActive.value = false
                        mutableCurrentTrackDurationSeconds.value = null
                        mutableControllerState.value = MediaControllerState.DISCONNECTED
                    }
                }
            }
        }
    }

    /**
     * Check if the MediaController is ready for operations.
     * Returns true if controller exists and is connected.
     * Note: We don't check playbackState == STATE_READY here because we need to be able
     * to set playWhenReady before the player reaches READY state (e.g., during IDLE or BUFFERING).
     */
    private fun isControllerReady(): Boolean {
        val controller = mediaController ?: return false
        return controller.isConnected
    }

    /**
     * Update the controller state based on current connection and playback status.
     */
    private fun updateControllerState() {
        val controller = mediaController
        when {
            controller == null -> mutableControllerState.value = MediaControllerState.DISCONNECTED
            !controller.isConnected -> mutableControllerState.value = MediaControllerState.CONNECTING
            else -> mutableControllerState.value = MediaControllerState.CONNECTED
        }
    }

    override fun setIsPlaying(isPlaying: Boolean) {
        // Always update our state - this ensures loadPlaylist uses the correct value
        mutableIsPlaying.value = isPlaying
        if (!isControllerReady()) {
            logger.info("setIsPlaying($isPlaying) - controller not ready, attempting to reconnect")
            reconnectAndExecute { controller ->
                controller.playWhenReady = isPlaying
            }
            return
        }
        mediaController!!.playWhenReady = isPlaying
    }

    override fun loadPlaylist(tracksUrls: List<String>) {
        logger.info("loadPlaylist() - ${tracksUrls.size} tracks, sessionToken=${sessionToken != null}")
        pendingTrackIndex = null
        if (sessionToken == null) {
            sessionToken =
                SessionToken(context, ComponentName(context, PlaybackService::class.java))
            mutableControllerState.value = MediaControllerState.CONNECTING
            val controllerFuture = MediaController.Builder(context, sessionToken!!).buildAsync()
            controllerFuture.addListener(
                {
                    mediaController = controllerFuture.get()
                    logger.info("MediaController created - isConnected=${mediaController?.isConnected}")
                    mediaController?.addListener(playerListener)
                    updateControllerState()
                    loadPlaylistWhenMediaControllerIsReady(tracksUrls)
                },
                MoreExecutors.directExecutor()
            )
        } else {
            loadPlaylistWhenMediaControllerIsReady(tracksUrls)
        }
    }

    private fun loadPlaylistWhenMediaControllerIsReady(tracksUrls: List<String>) {
        mediaController?.let {
            it.clearMediaItems()
            tracksUrls.forEach { url ->
                it.addMediaItem(MediaItem.fromUri(url))
            }
            it.prepare()
            it.playWhenReady = isPlaying.value
            mutableIsActive.value = true
            updateControllerState()
            if (isPlaying.value) {
                startProgressPolling()
            }
            // Apply pending track index if one was set before playlist was ready
            pendingTrackIndex?.let { index ->
                it.seekTo(index, 0)
                pendingTrackIndex = null
            }
        }
    }

    override fun loadTrackIndex(loadTrackIndex: Int) {
        if (mediaController != null) {
            mediaController?.seekTo(loadTrackIndex, 0)
        } else {
            // Store for later when mediaController is ready
            pendingTrackIndex = loadTrackIndex
        }
    }

    override fun togglePlayPause() {
        if (!isControllerReady()) {
            logger.info("togglePlayPause() - controller not ready, attempting to reconnect")
            reconnectAndExecute { controller ->
                controller.playWhenReady = !controller.playWhenReady
            }
            return
        }
        mediaController?.let { controller ->
            val newState = !controller.playWhenReady
            logger.info("togglePlayPause() - newState=$newState, isConnected=${controller.isConnected}, playbackState=${controller.playbackState}")
            controller.playWhenReady = newState
        }
    }

    /**
     * Reconnect to the PlaybackService and execute an action once connected.
     * This handles the case where the MediaController has disconnected but the service is still running.
     */
    private fun reconnectAndExecute(action: (MediaController) -> Unit) {
        // Release old controller if it exists but is disconnected
        mediaController?.let { controller ->
            if (!controller.isConnected) {
                logger.debug("Releasing disconnected controller")
                controller.removeListener(playerListener)
                controller.release()
                mediaController = null
            }
        }

        // Build a new controller
        val token = sessionToken ?: SessionToken(context, ComponentName(context, PlaybackService::class.java))
        sessionToken = token
        mutableControllerState.value = MediaControllerState.CONNECTING

        val controllerFuture = MediaController.Builder(context, token).buildAsync()
        controllerFuture.addListener(
            {
                val controller = controllerFuture.get()
                mediaController = controller
                logger.info("Reconnected MediaController - isConnected=${controller.isConnected}")
                controller.addListener(playerListener)
                updateControllerState()

                if (controller.isConnected) {
                    // Sync state from the reconnected controller
                    mutableIsPlaying.value = controller.playWhenReady
                    mutableIsActive.value = controller.mediaItemCount > 0
                    if (controller.playWhenReady) {
                        startProgressPolling()
                    }
                    action(controller)
                } else {
                    logger.warn("Reconnected but controller still not connected")
                }
            },
            MoreExecutors.directExecutor()
        )
    }

    override fun seekToPercentage(percentage: Float) {
        mediaController?.let { controller ->
            val duration = controller.duration
            if (duration > 0) {
                val position = (duration * percentage / 100f).toLong()
                controller.seekTo(position)
                emitSeekEvent()
            }
        }
    }

    override fun forward10Sec() {
        mediaController?.let { controller ->
            val newPosition = (controller.currentPosition + 10_000).coerceAtMost(controller.duration)
            controller.seekTo(newPosition)
            emitSeekEvent()
        }
    }

    override fun rewind10Sec() {
        mediaController?.let { controller ->
            val newPosition = (controller.currentPosition - 10_000).coerceAtLeast(0)
            controller.seekTo(newPosition)
            emitSeekEvent()
        }
    }

    private fun emitSeekEvent() {
        coroutineScope.launch {
            mutableSeekEvents.emit(SeekEvent(timestamp = System.currentTimeMillis()))
        }
    }

    override fun stop() {
        mediaController?.stop()
        mutableIsPlaying.value = false
        mutableCurrentTrackDurationSeconds.value = null
        stopProgressPolling()
    }

    override fun clearSession() {
        stopProgressPolling()
        autoRetryJob?.cancel()
        autoRetryJob = null
        mediaController?.stop()
        mediaController?.clearMediaItems()
        mutableIsActive.value = false
        mutableIsPlaying.value = false
        mutableCurrentTrackIndex.value = null
        mutableCurrentTrackPercent.value = null
        mutableCurrentTrackProgressSec.value = null
        mutableCurrentTrackDurationSeconds.value = null
        mutablePlayerError.value = null
    }

    override fun setVolume(volume: Float) {
        mediaController?.volume = volume.coerceIn(0f, 1f)
        mutableVolumeState.value = mutableVolumeState.value.copy(volume = volume.coerceIn(0f, 1f))
    }

    override fun setMuted(isMuted: Boolean) {
        mediaController?.volume = if (isMuted) 0f else mutableVolumeState.value.volume
        mutableVolumeState.value = mutableVolumeState.value.copy(isMuted = isMuted)
    }

    override fun skipToNextTrack() {
        mediaController?.seekToNext()
    }

    override fun skipToPreviousTrack() {
        mediaController?.seekToPrevious()
    }

    override fun addMediaItems(tracksUrls: List<String>) {
        mediaController?.let { controller ->
            tracksUrls.forEach { url ->
                controller.addMediaItem(MediaItem.fromUri(url))
            }
        }
    }

    override fun removeMediaItem(index: Int) {
        mediaController?.removeMediaItem(index)
    }

    override fun toggleShuffle() {
        val newShuffleEnabled = !mutableShuffleEnabled.value
        mutableShuffleEnabled.value = newShuffleEnabled
        mediaController?.shuffleModeEnabled = newShuffleEnabled
    }

    override fun cycleRepeatMode() {
        val newMode = when (mutableRepeatMode.value) {
            RepeatMode.OFF -> RepeatMode.ALL
            RepeatMode.ALL -> RepeatMode.ONE
            RepeatMode.ONE -> RepeatMode.OFF
        }
        mutableRepeatMode.value = newMode
        mediaController?.repeatMode = when (newMode) {
            RepeatMode.OFF -> Player.REPEAT_MODE_OFF
            RepeatMode.ALL -> Player.REPEAT_MODE_ALL
            RepeatMode.ONE -> Player.REPEAT_MODE_ONE
        }
    }

    /**
     * Classify an error as transient (recoverable) or permanent.
     * Transient errors are those that might resolve on retry (e.g., network issues).
     * Permanent errors are those that won't be fixed by retry (e.g., codec errors).
     */
    private fun isTransientError(error: PlaybackException): Boolean {
        return when (error.errorCode) {
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
            PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS -> true

            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
            PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED -> false

            else -> false // Default to permanent for unknown errors
        }
    }

    /**
     * Internal retry implementation.
     * Clears error state and resumes playback from last known position.
     */
    private fun retryInternal() {
        mediaController?.let { controller ->
            val positionMs = lastKnownPositionMs.takeIf { it > 0 } ?: 0L
            logger.info("Retrying playback from position: ${positionMs}ms")
            // After a PlaybackException, ExoPlayer transitions to STATE_IDLE.
            // We must call prepare() to restart playback.
            controller.prepare()
            controller.seekTo(positionMs)
            controller.playWhenReady = true
            // Error state will be cleared when playback reaches READY state
        }
    }

    /**
     * Retry playback after an error.
     * Resumes from the last position if available.
     */
    override fun retry() {
        if (mutablePlayerError.value != null) {
            logger.info("Manual retry requested by user")
            retryInternal()
        } else {
            logger.debug("Retry requested but no error present")
        }
    }
}