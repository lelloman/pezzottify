package com.lelloman.pezzottify.android.localplayer

import android.content.Context
import android.net.Uri
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
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
    context: Context,
    private val coroutineScope: CoroutineScope
) {
    private val exoPlayer: ExoPlayer = ExoPlayer.Builder(context)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                .build(),
            true // handle audio focus
        )
        .build()

    private val _state = MutableStateFlow(LocalPlayerState())
    val state: StateFlow<LocalPlayerState> = _state.asStateFlow()

    private var progressPollingJob: Job? = null
    private var trackInfoList: List<LocalTrackInfo> = emptyList()

    private val playerListener = object : Player.Listener {
        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            _state.update { it.copy(isPlaying = playWhenReady) }
            updateProgressPolling(playWhenReady)
        }

        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            _state.update { it.copy(currentTrackIndex = exoPlayer.currentMediaItemIndex) }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            if (playbackState == Player.STATE_READY) {
                updateDuration()
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            // Log error but continue - could skip to next track
        }
    }

    init {
        exoPlayer.addListener(playerListener)
    }

    fun loadQueue(tracks: List<LocalTrackInfo>, startIndex: Int = 0) {
        trackInfoList = tracks
        exoPlayer.clearMediaItems()

        tracks.forEach { track ->
            exoPlayer.addMediaItem(MediaItem.fromUri(Uri.parse(track.uri)))
        }

        _state.update {
            it.copy(
                queue = tracks,
                currentTrackIndex = startIndex,
                progressPercent = 0f,
                progressSeconds = 0,
                durationSeconds = 0
            )
        }

        if (tracks.isNotEmpty()) {
            exoPlayer.seekTo(startIndex, 0)
            exoPlayer.prepare()
            exoPlayer.playWhenReady = true
        }
    }

    fun addToQueue(tracks: List<LocalTrackInfo>) {
        trackInfoList = trackInfoList + tracks
        tracks.forEach { track ->
            exoPlayer.addMediaItem(MediaItem.fromUri(Uri.parse(track.uri)))
        }
        _state.update { it.copy(queue = trackInfoList) }
    }

    fun play() {
        exoPlayer.playWhenReady = true
    }

    fun pause() {
        exoPlayer.playWhenReady = false
    }

    fun togglePlayPause() {
        exoPlayer.playWhenReady = !exoPlayer.playWhenReady
    }

    fun seekToPercent(percent: Float) {
        val duration = exoPlayer.duration
        if (duration > 0) {
            val position = (duration * percent / 100f).toLong()
            exoPlayer.seekTo(position)
            updateProgress()
        }
    }

    fun skipNext() {
        if (exoPlayer.hasNextMediaItem()) {
            exoPlayer.seekToNext()
        }
    }

    fun skipPrevious() {
        // If more than 3 seconds into track, restart it; otherwise go to previous
        if (exoPlayer.currentPosition > 3000) {
            exoPlayer.seekTo(0)
        } else if (exoPlayer.hasPreviousMediaItem()) {
            exoPlayer.seekToPrevious()
        } else {
            exoPlayer.seekTo(0)
        }
    }

    fun selectTrack(index: Int) {
        if (index in 0 until exoPlayer.mediaItemCount) {
            exoPlayer.seekTo(index, 0)
        }
    }

    fun release() {
        stopProgressPolling()
        exoPlayer.removeListener(playerListener)
        exoPlayer.release()
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
        val duration = exoPlayer.duration
        val position = exoPlayer.currentPosition
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
        val duration = exoPlayer.duration
        if (duration > 0) {
            _state.update { it.copy(durationSeconds = (duration / 1000).toInt()) }
        }
    }
}
