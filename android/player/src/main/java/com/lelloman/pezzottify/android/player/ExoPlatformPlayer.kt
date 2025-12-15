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

    private val playerListener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            super.onEvents(player, events)
            for (i in 0 until events.size()) {
                when (events.get(i)) {
                    Player.EVENT_PLAY_WHEN_READY_CHANGED -> {
                        mutableIsPlaying.value = player.playWhenReady
                        updateProgressPolling(player.playWhenReady)
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
        }

        override fun onPlayerError(error: PlaybackException) {
            super.onPlayerError(error)
            logger.error("Player error: ${error.errorCodeName} - ${error.message}", error)
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
                    }
                }
            }
        }
    }

    override fun setIsPlaying(isPlaying: Boolean) {
        val controller = mediaController
        logger.info("setIsPlaying($isPlaying) - controller=${controller != null}, isConnected=${controller?.isConnected}")
        controller?.playWhenReady = isPlaying
    }

    override fun loadPlaylist(tracksUrls: List<String>) {
        logger.info("loadPlaylist() - ${tracksUrls.size} tracks, sessionToken=${sessionToken != null}")
        mutableIsPlaying.value = true
        pendingTrackIndex = null
        if (sessionToken == null) {
            sessionToken =
                SessionToken(context, ComponentName(context, PlaybackService::class.java))
            val controllerFuture = MediaController.Builder(context, sessionToken!!).buildAsync()
            controllerFuture.addListener(
                {
                    mediaController = controllerFuture.get()
                    logger.info("MediaController created - isConnected=${mediaController?.isConnected}")
                    mediaController?.addListener(playerListener)
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
            mediaController?.clearMediaItems()
            tracksUrls.forEach {
                mediaController?.addMediaItem(MediaItem.fromUri(it))
            }
            mediaController?.prepare()
            mediaController?.playWhenReady = isPlaying.value
            mutableIsActive.value = true
            if (isPlaying.value) {
                startProgressPolling()
            }
            // Apply pending track index if one was set before playlist was ready
            pendingTrackIndex?.let { index ->
                mediaController?.seekTo(index, 0)
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
        val controller = mediaController
        val newState = !isPlaying.value
        logger.info("togglePlayPause() - newState=$newState, controller=${controller != null}, isConnected=${controller?.isConnected}, playbackState=${controller?.playbackState}")
        mutableIsPlaying.value = newState
        controller?.playWhenReady = newState
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
        mediaController?.stop()
        mediaController?.clearMediaItems()
        mutableIsActive.value = false
        mutableIsPlaying.value = false
        mutableCurrentTrackIndex.value = null
        mutableCurrentTrackPercent.value = null
        mutableCurrentTrackProgressSec.value = null
        mutableCurrentTrackDurationSeconds.value = null
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
}