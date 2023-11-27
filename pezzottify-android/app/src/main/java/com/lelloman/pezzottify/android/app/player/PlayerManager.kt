package com.lelloman.pezzottify.android.app.player

import android.content.Context
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.lelloman.pezzottify.android.app.domain.LogoutOperation
import com.lelloman.pezzottify.android.localdata.model.Playlist
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

interface PlayerManager {

    val state: StateFlow<State>

    fun play(playList: Playlist)

    suspend fun dispose()

    sealed class State {
        object Off : State()

        data class Playing(
            val paused: Boolean,
            val trackName: String,
            val artistName: String,
            val trackDurationMs: Long,
            val currentTimeMs: Long
        ) : State()
    }
}

internal class PlayerManagerImpl(
    private val context: Context,
    private val playerDispatcher: CoroutineDispatcher,
    private val authTokenProvider: Flow<String>,
) : PlayerManager, Player.Listener, LogoutOperation {

    private val mutableState = MutableStateFlow<PlayerManager.State>(PlayerManager.State.Off)
    override val state = mutableState.asStateFlow()

    private val playerHolder = PlayerHolder()

    override fun play(playList: Playlist) {
        GlobalScope.launch {
            withContext(playerDispatcher) {
                playerHolder.getPlayer(authTokenProvider.firstOrNull() ?: "").let { player ->
                    playList.audioTracksIds.forEach { audioTrackId ->
                        val url = "http://10.0.2.2:8080/api/track/${audioTrackId}"
                        player.addMediaItem(MediaItem.fromUri(url))
                    }
                    player.prepare()
                    player.playWhenReady = true
                }
            }
        }
    }

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        super.onIsPlayingChanged(isPlaying)
        val prev = state.value
        val newValue = when (prev) {
            is PlayerManager.State.Off -> PlayerManager.State.Playing(!isPlaying, "", "", 0, 0)
            is PlayerManager.State.Playing -> prev.copy(paused = !isPlaying)
        }
        mutableState.tryEmit(newValue)
    }

    override fun onPlaybackStateChanged(playbackState: Int) {
        super.onPlaybackStateChanged(playbackState)
        Log.d("ASDASD", "onPlaybackStateChanged() $playbackState")
    }

    private inner class PlayerHolder {
        private var player: ExoPlayer? = null

        @OptIn(UnstableApi::class)
        suspend fun getPlayer(authToken: String): ExoPlayer {
            player?.let { return it }
            return withContext(playerDispatcher) {
                val dataSourceFactory = DataSource.Factory {
                    DefaultHttpDataSource.Factory().createDataSource().apply {
                        this.setRequestProperty("Authorization", "Bearer $authToken")
                    }
                }
                val mediaSourceFactory = DefaultMediaSourceFactory(dataSourceFactory)
                ExoPlayer.Builder(context).setMediaSourceFactory(mediaSourceFactory).build()
                    .apply {
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

    override suspend fun invoke() {
        TODO("Not yet implemented")
    }

    override suspend fun dispose() {
        playerHolder.dispose()
    }
}