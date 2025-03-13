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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

@OptIn(DelicateCoroutinesApi::class)
@UnstableApi
internal class ExoPlatformPlayer(
    private val context: Context,
    playerServiceEventsEmitter: PlayerServiceEventsEmitter,
    coroutineScope: CoroutineScope = GlobalScope,
) : PlatformPlayer {

    private var mediaController: MediaController? = null

    private var sessionToken: SessionToken? = null

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

    init {
        coroutineScope.launch(Dispatchers.Main) {
            playerServiceEventsEmitter.events.collect {
                when (it) {
                    PlayerServiceEventsEmitter.Event.Shutdown -> {
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
        }
    }

    override fun loadTrackIndex(loadTrackIndex: Int) {
        mediaController?.seekTo(loadTrackIndex, 0)
    }

    override fun togglePlayPause() {
        mutableIsPlaying.value = !isPlaying.value
        mediaController?.playWhenReady = isPlaying.value
    }

    override fun seekToPercentage(percentage: Float) {
        TODO("Not yet implemented")
    }

    override fun forward10Sec() {
        TODO("Not yet implemented")
    }

    override fun rewind10Sec() {
        TODO("Not yet implemented")
    }

    override fun stop() {
        TODO("Not yet implemented")
    }

    override fun setVolume(volume: Float) {
        TODO("Not yet implemented")
    }

    override fun setMuted(isMuted: Boolean) {
        TODO("Not yet implemented")
    }

    override fun skipToNextTrack() {
        mediaController?.seekToNext()
    }

    override fun skipToPreviousTrack() {
        mediaController?.seekToPrevious()
    }
}