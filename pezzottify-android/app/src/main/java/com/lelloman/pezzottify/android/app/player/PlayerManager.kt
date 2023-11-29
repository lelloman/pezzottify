package com.lelloman.pezzottify.android.app.player

import android.content.ComponentName
import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.Player.EVENT_PLAYBACK_STATE_CHANGED
import androidx.media3.common.Player.EVENT_PLAY_WHEN_READY_CHANGED
import androidx.media3.common.Player.EVENT_TRACKS_CHANGED
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.lelloman.pezzottify.android.app.ui.PlaybackService
import com.lelloman.pezzottify.android.localdata.model.AlbumWithTracks
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
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
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlin.math.roundToLong

interface PlayerManager {

    val state: StateFlow<State>

    fun play(playList: AlbumWithTracks)

    suspend fun <T> withPlayer(action: (ExoPlayer) -> T): T

    fun togglePlayPause()

    fun seek(percent: Float)

    fun seekToNext()

    fun seekToPrevious()

    fun getPlayer(): ExoPlayer

    suspend fun dispose()

    sealed class State {
        object Off : State()

        data class Playing(
            val isPlaying: Boolean = false,
            val trackName: String = "",
            val trackImageId: String? = null,
            val artistName: String = "",
            val albumName: String = "",
            val trackDurationMs: Long = 0L,
            val currentPositionMs: Long = 0L,
        ) : State()
    }
}

@UnstableApi
internal class PlayerManagerImpl(
    private val context: Context,
    private val playerDispatcher: CoroutineDispatcher,
    private val authTokenProvider: Flow<String>,
    loggerFactory: LoggerFactory,
) : PlayerManager, Player.Listener {

    private val log by loggerFactory

    private val mutableState = MutableStateFlow<PlayerManager.State>(PlayerManager.State.Off)
    override val state = mutableState.asStateFlow()

    private var _player: ExoPlayer? = null

    private var authToken: String? = null
    private var pollProgressJob: Job? = null

    private var currentPlaylist: AlbumWithTracks? = null

    private val dataSourceFactory = DataSource.Factory {
        DefaultHttpDataSource.Factory().createDataSource().apply {
            this.setRequestProperty(
                "Authorization", "Bearer ${authToken}"
            )
        }
    }
    private val mediaSourceFactory = DefaultMediaSourceFactory(dataSourceFactory)

    init {
        GlobalScope.launch {
            authTokenProvider.collect { authToken = it }
        }
    }

    private fun playerOperation(operation: (ExoPlayer) -> Unit) {
        GlobalScope.launch {
            withContext(playerDispatcher) {
                operation(getOrMakePlayer())
            }
        }
    }

    override suspend fun <T> withPlayer(action: (ExoPlayer) -> T): T = withContext(playerDispatcher) {
        action(getOrMakePlayer())
    }

    private fun updatePlayingState(action: (PlayerManager.State.Playing) -> PlayerManager.State.Playing) {
        val oldState: PlayerManager.State.Playing =
            state.value.takeIf { it is PlayerManager.State.Playing }
                ?.let { it as PlayerManager.State.Playing } ?: PlayerManager.State.Playing()
        mutableState.tryEmit(action(oldState))
    }

    override fun play(playList: AlbumWithTracks) = playerOperation { player ->
        currentPlaylist = playList
        player.clearMediaItems()
        playList.tracks.forEach { audioTrack ->
            val url = "http://10.0.2.2:8080/api/track/${audioTrack.id}"
            val tag = AudioTrackTag(playList, audioTrack)
            val metadata =
                MediaMetadata.Builder().setAlbumArtist("THE ARTIST").setTitle(audioTrack.name)
                    .build()
            val mediaItem =
                MediaItem.Builder().setUri(url).setTag(tag).setMediaMetadata(metadata).build()
            player.addMediaItem(mediaItem)
        }
        updatePlayingState { it.copy(albumName = playList.album.name) }
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
            ?.let { it as PlayerManager.State.Playing }?.copy(currentPositionMs = positionMs)
            ?.let(mutableState::tryEmit)
    }

    override fun togglePlayPause() = playerOperation {
        if (it.playWhenReady) it.pause() else it.play()
    }

    private fun startPollDurationJob() {
        pollProgressJob = GlobalScope.launch {
            playerDispatcher.run {
                while (true) {
                    playerOperation { player ->
                        updatePlayingState {
                            it.copy(
                                trackDurationMs = player.duration,
                                currentPositionMs = player.currentPosition,
                            )
                        }
                    }
                    delay(500)
                }
            }
        }
    }

    override fun getPlayer(): ExoPlayer = runBlocking { getOrMakePlayer() }

    override fun onEvents(player: Player, events: Player.Events) {
        log.debug("onEvents() $events")
        val playbackStateChanged =
            events.containsAny(EVENT_PLAYBACK_STATE_CHANGED, EVENT_PLAY_WHEN_READY_CHANGED)
        val trackChanged = events.contains(EVENT_TRACKS_CHANGED)
        if (!playbackStateChanged && !trackChanged) return

        updatePlayingState {
            var newState = it
            if (playbackStateChanged) {
                val shouldShowPlay =
                    !player.playWhenReady || player.playbackState == Player.STATE_IDLE || player.playbackState == Player.STATE_ENDED
                val isPlaying = !shouldShowPlay
                newState = newState.copy(isPlaying = isPlaying)
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
            if (trackChanged) {
                val track = player.currentMediaItem?.localConfiguration?.tag
                if (track is AudioTrackTag) {
                    newState = newState.copy(
                        trackName = track.name,
                        trackImageId = track.imageId,
                    )
                    log.debug("New track name ${track.name}")
                }
            }
            newState
        }
    }

    private fun buildMediaController() {
        val sessionToken =
            SessionToken(context, ComponentName(context, PlaybackService::class.java))
        MediaController.Builder(context, sessionToken).buildAsync()
    }

    private suspend fun getOrMakePlayer() = _player ?: withContext(playerDispatcher) {
        ExoPlayer.Builder(context).setMediaSourceFactory(mediaSourceFactory).build()
            .also { player ->
                this@PlayerManagerImpl._player = player
                player.addListener(this@PlayerManagerImpl)
                player.playWhenReady = false
                buildMediaController()
            }
    }

    override suspend fun dispose() {
        withContext(playerDispatcher) {
            this@PlayerManagerImpl._player?.let { player ->
                player.stop()
                player.release()
                this@PlayerManagerImpl._player = null
            }
        }
        mutableState.emit(PlayerManager.State.Off)
    }

    private data class AudioTrackTag(
        val name: String,
        val imageId: String?,
    ) {
        constructor(album: AlbumWithTracks, audioTrack: AudioTrack) : this(
            name = audioTrack.name,
            imageId = album.album.coverImageId,
        )
    }
}