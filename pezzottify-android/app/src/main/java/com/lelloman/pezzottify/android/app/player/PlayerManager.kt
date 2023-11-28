package com.lelloman.pezzottify.android.app.player

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.Player.EVENT_PLAYBACK_STATE_CHANGED
import androidx.media3.common.Player.EVENT_PLAY_WHEN_READY_CHANGED
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.lelloman.pezzottify.android.localdata.model.Playlist
import com.lelloman.pezzottify.android.log.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.roundToLong

interface PlayerManager {

    val state: StateFlow<State>

    fun play(playList: Playlist)

    fun togglePlayPause()

    fun seek(percent: Float)

    fun seekToNext()

    fun seekToPrevious()

    suspend fun dispose()

    sealed class State {
        object Off : State()

        data class Playing(
            val isPlaying: Boolean = false,
            val trackName: String = "",
            val artistName: String = "",
            val trackDurationMs: Long = 0L,
            val currentPositionMs: Long = 0L,
        ) : State()
    }
}

internal class PlayerManagerImpl(
    private val context: Context,
    private val playerDispatcher: CoroutineDispatcher,
    private val authTokenProvider: Flow<String>,
    loggerFactory: LoggerFactory,
) : PlayerManager, Player.Listener {

    private val log by loggerFactory

    private val mutableState = MutableStateFlow<PlayerManager.State>(PlayerManager.State.Off)
    override val state = mutableState.asStateFlow()

    private val playerHolder = PlayerHolder()
    private var authToken: String? = null
    private var pollProgressJob: Job? = null

    init {
        GlobalScope.launch {
            authTokenProvider.collect { authToken = it }
        }
    }

    private fun playerOperation(operation: (ExoPlayer) -> Unit) {
        GlobalScope.launch {
            withContext(playerDispatcher) {
                operation(playerHolder.getPlayer())
            }
        }
    }

    override fun play(playList: Playlist) = playerOperation { player ->
        player.clearMediaItems()
        playList.audioTracksIds.forEach { audioTrackId ->
            val url = "http://10.0.2.2:8080/api/track/${audioTrackId}"
            player.addMediaItem(MediaItem.fromUri(url))
        }
        player.prepare()
        player.playWhenReady = true
    }

    override fun seek(percent: Float) = playerOperation { player ->
        val positionMs = player.duration.toDouble().times(percent.toDouble()).roundToLong()
        player.seekTo(positionMs)
        emitNewPositionMs(positionMs)
    }

    override fun seekToNext() = playerOperation { player ->
        player.seekToNext()
        emitNewPositionMs(0)
    }

    override fun seekToPrevious() = playerOperation { player ->
        player.seekToPrevious()
        emitNewPositionMs(0)
    }

    private fun emitNewPositionMs(positionMs: Long) {
        state.value.takeIf { it is PlayerManager.State.Playing }
            ?.let { it as PlayerManager.State.Playing }
            ?.copy(currentPositionMs = positionMs)
            ?.let(mutableState::tryEmit)
    }

    override fun togglePlayPause() = playerOperation {
        if (it.playWhenReady) it.pause() else it.play()
    }

    private fun startPollDurationJob() {
        pollProgressJob = GlobalScope.launch {
            playerDispatcher.run {
                while (true) {
                    val currentState = state.value
                    if (currentState !is PlayerManager.State.Playing) break
                    playerOperation {
                        val newState = currentState.copy(
                            trackDurationMs = it.duration,
                            currentPositionMs = it.currentPosition,
                        )
                        mutableState.tryEmit(newState)
                    }
                    delay(500)
                }
            }
        }
    }

    override fun onEvents(player: Player, events: Player.Events) {
        if (events.containsAny(EVENT_PLAYBACK_STATE_CHANGED, EVENT_PLAY_WHEN_READY_CHANGED)) {
            val shouldShowPlay =
                !player.playWhenReady || player.playbackState == Player.STATE_IDLE || player.playbackState == Player.STATE_ENDED
            val isPlaying = !shouldShowPlay
            val newValue = when (val prev = state.value) {
                is PlayerManager.State.Off -> PlayerManager.State.Playing(isPlaying = isPlaying)

                is PlayerManager.State.Playing -> prev.copy(isPlaying = isPlaying)
            }
            mutableState.tryEmit(newValue)
            if (!isPlaying) {
                pollProgressJob?.cancel()
                pollProgressJob = null
            } else if (pollProgressJob == null) {
                log.debug("onIsPlayingChanged() starting poll progress job.")
                startPollDurationJob()
            } else {
                log.warn("onIsPlayingChanged() NOT starting poll progress job as it's already running, this shouldn't be happening.")
            }
        }
    }

    private inner class PlayerHolder {
        private var player: ExoPlayer? = null

        @OptIn(UnstableApi::class)
        suspend fun getPlayer(): ExoPlayer {
            player?.let { return it }
            return withContext(playerDispatcher) {
                val dataSourceFactory = DataSource.Factory {
                    DefaultHttpDataSource.Factory().createDataSource().apply {
                        this.setRequestProperty(
                            "Authorization", "Bearer ${this@PlayerManagerImpl.authToken}"
                        )
                    }
                }
                val mediaSourceFactory = DefaultMediaSourceFactory(dataSourceFactory)
                ExoPlayer.Builder(context).setMediaSourceFactory(mediaSourceFactory).build().apply {
                    player = this
                    addListener(this@PlayerManagerImpl)
                }
            }
        }

        suspend fun dispose() {
            withContext(playerDispatcher) {
                player?.let { player ->
                    player.stop()
                    player.release()
                    this@PlayerHolder.player = null
                }
            }
        }
    }

    override suspend fun dispose() {
        playerHolder.dispose()
        mutableState.emit(PlayerManager.State.Off)
    }
}