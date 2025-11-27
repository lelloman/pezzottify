package com.lelloman.pezzottify.android.player

import android.content.ComponentName
import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.Log
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.VolumeState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

@OptIn(DelicateCoroutinesApi::class)
@UnstableApi
internal class ExoPlatformPlayer(
    private val context: Context,
    playerServiceEventsEmitter: PlayerServiceEventsEmitter,
    private val coroutineScope: CoroutineScope = GlobalScope,
) : PlatformPlayer {

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


    private val playerListener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            super.onEvents(player, events)
            Log.d("ASDASD", "ExoPlatformPlayer: onEvents $events")
            for (i in 0 until events.size()) {
                val event = events.get(i)
                val eventName = when (event) {
                    Player.EVENT_PLAY_WHEN_READY_CHANGED -> {
                        mutableIsPlaying.value = player.playWhenReady
                        updateProgressPolling(player.playWhenReady)
                        "EVENT_PLAY_WHEN_READY_CHANGED"
                    }

                    Player.EVENT_POSITION_DISCONTINUITY -> {
                        mutableCurrentTrackIndex.value = player.currentMediaItemIndex
                        "EVENT_POSITION_DISCONTINUITY"
                    }

                    else -> "$event"
                }
                Log.d("ASDASD", "events has: $eventName")
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
            Log.d("ASDASD", "ExoPlatformPlayer: onPlaybackStateChanged $playbackStateText")
        }

        override fun onAudioSessionIdChanged(audioSessionId: Int) {
            super.onAudioSessionIdChanged(audioSessionId)
            Log.d("ASDASD", "ExoPlatformPlayer: audioSessionIdChanged $audioSessionId")
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
        }
    }

    init {
        coroutineScope.launch(Dispatchers.Main) {
            playerServiceEventsEmitter.events.collect {
                when (it) {
                    PlayerServiceEventsEmitter.Event.Shutdown -> {
                        stopProgressPolling()
                        mediaController?.removeListener(playerListener)
                        mediaController?.release()
                        mediaController = null
                        sessionToken = null
                        mutableIsActive.value = false
                    }
                }
            }
        }
    }

    override fun setIsPlaying(isPlaying: Boolean) {
        mediaController?.playWhenReady = isPlaying
    }

    override fun loadPlaylist(tracksUrls: List<String>) {
        mutableIsPlaying.value = true
        pendingTrackIndex = null
        if (sessionToken == null) {
            sessionToken =
                SessionToken(context, ComponentName(context, PlaybackService::class.java))
            val controllerFuture = MediaController.Builder(context, sessionToken!!).buildAsync()
            controllerFuture.addListener(
                {
                    mediaController = controllerFuture.get()
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
        mutableIsPlaying.value = !isPlaying.value
        mediaController?.playWhenReady = isPlaying.value
    }

    override fun seekToPercentage(percentage: Float) {
        mediaController?.let { controller ->
            val duration = controller.duration
            if (duration > 0) {
                val position = (duration * percentage / 100f).toLong()
                controller.seekTo(position)
            }
        }
    }

    override fun forward10Sec() {
        mediaController?.let { controller ->
            val newPosition = (controller.currentPosition + 10_000).coerceAtMost(controller.duration)
            controller.seekTo(newPosition)
        }
    }

    override fun rewind10Sec() {
        mediaController?.let { controller ->
            val newPosition = (controller.currentPosition - 10_000).coerceAtLeast(0)
            controller.seekTo(newPosition)
        }
    }

    override fun stop() {
        mediaController?.stop()
        mutableIsPlaying.value = false
        stopProgressPolling()
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
}