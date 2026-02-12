package com.lelloman.pezzottify.android.localplayer

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class LocalExoPlayer(
    private val context: Context,
    private val coroutineScope: CoroutineScope
) {
    private var mediaController: MediaController? = null
    private var sessionToken: SessionToken? = null

    private val _state = MutableStateFlow(LocalPlayerState())
    val state: StateFlow<LocalPlayerState> = _state.asStateFlow()

    private var progressPollingJob: Job? = null
    private var trackInfoList: List<LocalTrackInfo> = emptyList()

    // Queue actions until controller is ready
    private var pendingPlayWhenReady: Boolean? = null

    private val playerListener = object : Player.Listener {
        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            _state.update { it.copy(isPlaying = playWhenReady) }
            updateProgressPolling(playWhenReady)
        }

        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            val controller = mediaController ?: return
            _state.update { it.copy(currentTrackIndex = controller.currentMediaItemIndex) }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            if (playbackState == Player.STATE_READY) {
                updateDuration()
                // Update progress immediately when ready (important for restore)
                updateProgress()
            }
        }
    }

    fun loadQueue(tracks: List<LocalTrackInfo>, startIndex: Int = 0) {
        trackInfoList = tracks
        _state.update {
            it.copy(
                queue = tracks,
                currentTrackIndex = startIndex,
                progressPercent = 0f,
                progressSeconds = 0,
                durationSeconds = 0
            )
        }

        if (tracks.isEmpty()) return

        ensureMediaController { controller ->
            controller.clearMediaItems()
            tracks.forEach { track ->
                val mediaItem = MediaItem.Builder()
                    .setUri(Uri.parse(track.uri))
                    .setMediaMetadata(
                        MediaMetadata.Builder()
                            .setTitle(track.displayName)
                            .build()
                    )
                    .build()
                controller.addMediaItem(mediaItem)
            }
            controller.seekTo(startIndex, 0)
            controller.prepare()
            controller.playWhenReady = true
        }
    }

    fun addToQueue(tracks: List<LocalTrackInfo>) {
        if (tracks.isEmpty()) return

        trackInfoList = trackInfoList + tracks
        _state.update { it.copy(queue = trackInfoList) }

        mediaController?.let { controller ->
            tracks.forEach { track ->
                val mediaItem = MediaItem.Builder()
                    .setUri(Uri.parse(track.uri))
                    .setMediaMetadata(
                        MediaMetadata.Builder()
                            .setTitle(track.displayName)
                            .build()
                    )
                    .build()
                controller.addMediaItem(mediaItem)
            }
        }
    }

    fun restoreState(tracks: List<LocalTrackInfo>, currentIndex: Int, positionMs: Long) {
        if (tracks.isEmpty()) return

        trackInfoList = tracks
        _state.update {
            it.copy(
                queue = tracks,
                currentTrackIndex = currentIndex,
                progressPercent = 0f,
                progressSeconds = (positionMs / 1000).toInt(),
                durationSeconds = 0
            )
        }

        ensureMediaController { controller ->
            controller.clearMediaItems()
            tracks.forEach { track ->
                val mediaItem = MediaItem.Builder()
                    .setUri(Uri.parse(track.uri))
                    .setMediaMetadata(
                        MediaMetadata.Builder()
                            .setTitle(track.displayName)
                            .build()
                    )
                    .build()
                controller.addMediaItem(mediaItem)
            }
            controller.seekTo(currentIndex, positionMs)
            controller.prepare()
            // Don't auto-play on restore
            controller.playWhenReady = false
        }
    }

    fun play() {
        val controller = mediaController
        if (controller != null) {
            controller.playWhenReady = true
        } else {
            // Controller not ready yet - queue the action
            pendingPlayWhenReady = true
        }
    }

    fun pause() {
        val controller = mediaController
        if (controller != null) {
            controller.playWhenReady = false
        } else {
            pendingPlayWhenReady = false
        }
    }

    fun togglePlayPause() {
        val controller = mediaController
        if (controller != null) {
            controller.playWhenReady = !controller.playWhenReady
        } else {
            // Controller not ready yet - queue play action
            pendingPlayWhenReady = !(pendingPlayWhenReady ?: false)
        }
    }

    fun seekToPercent(percent: Float) {
        mediaController?.let { controller ->
            val duration = controller.duration
            if (duration > 0) {
                val position = (duration * percent / 100f).toLong()
                controller.seekTo(position)
                updateProgress()
            }
        }
    }

    fun skipNext() {
        mediaController?.let {
            if (it.hasNextMediaItem()) {
                it.seekToNext()
            }
        }
    }

    fun skipPrevious() {
        mediaController?.let { controller ->
            // If more than 3 seconds into track, restart it; otherwise go to previous
            if (controller.currentPosition > 3000) {
                controller.seekTo(0)
            } else if (controller.hasPreviousMediaItem()) {
                controller.seekToPrevious()
            } else {
                controller.seekTo(0)
            }
        }
    }

    fun selectTrack(index: Int) {
        mediaController?.let { controller ->
            if (index in 0 until controller.mediaItemCount) {
                controller.seekTo(index, 0)
            }
        }
    }

    fun release() {
        stopProgressPolling()
        mediaController?.removeListener(playerListener)
        mediaController?.release()
        mediaController = null
        sessionToken = null
    }

    private fun ensureMediaController(onReady: (MediaController) -> Unit) {
        mediaController?.let { controller ->
            onReady(controller)
            return
        }

        val newSessionToken = sessionToken ?: SessionToken(
            context,
            ComponentName(context, LocalPlaybackService::class.java)
        ).also { sessionToken = it }

        val controllerFuture = MediaController.Builder(context, newSessionToken).buildAsync()
        controllerFuture.addListener(
            {
                val controller = controllerFuture.get()
                mediaController = controller
                controller.addListener(playerListener)
                onReady(controller)

                // Apply any pending play/pause action
                pendingPlayWhenReady?.let { shouldPlay ->
                    controller.playWhenReady = shouldPlay
                    pendingPlayWhenReady = null
                }

                // Start polling if already playing
                if (controller.playWhenReady) {
                    startProgressPolling()
                }
            },
            MoreExecutors.directExecutor()
        )
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
            _state.update {
                it.copy(
                    progressPercent = percent,
                    progressSeconds = (position / 1000).toInt(),
                    durationSeconds = (duration / 1000).toInt()
                )
            }
        }
    }

    private fun updateDuration() {
        val controller = mediaController ?: return
        val duration = controller.duration
        if (duration > 0) {
            _state.update { it.copy(durationSeconds = (duration / 1000).toInt()) }
        }
    }
}
