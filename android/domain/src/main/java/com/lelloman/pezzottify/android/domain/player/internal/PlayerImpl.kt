package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.Player
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.newSingleThreadContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.CoroutineContext
import kotlin.time.Duration.Companion.seconds

@OptIn(DelicateCoroutinesApi::class)
internal class PlayerImpl(
    private val staticsProvider: StaticsProvider,
    loggerFactory: LoggerFactory,
    private val coroutineContext: CoroutineContext = newSingleThreadContext("PlayerImplThread"),
    private val coroutineScope: CoroutineScope = GlobalScope,
) : Player {

    private val logger by loggerFactory

    private var loadNexPlaylistJob: Job? = null

    private val mutablePlaybackPlaylist = MutableStateFlow<PlaybackPlaylist?>(null)
    override val playbackPlaylist = mutablePlaybackPlaylist.asStateFlow()

    private val mutableIsPlaying = MutableStateFlow(false)
    override val isPlaying: StateFlow<Boolean> = mutableIsPlaying.asStateFlow()

    private val mutableVolumeState = MutableStateFlow(VolumeState(0.5f, false))
    override val volumeState: StateFlow<VolumeState> = mutableVolumeState.asStateFlow()

    override val canGoToPreviousPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")

    override val canGoToNextPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")

    override fun loadAlbum(albumId: String) {
        coroutineScope.launch(coroutineContext) {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = launch {

                val loadedAlbum = withTimeoutOrNull(2.seconds) {
                    staticsProvider.provideAlbum(albumId)
                        .filterIsInstance<StaticsItem.Loaded<Album>>()
                        .first()
                }
                if (loadedAlbum != null) {
                    mutablePlaybackPlaylist.value = PlaybackPlaylist(
                        context = PlaybackPlaylistContext.Album(albumId),
                        tracksIds = loadedAlbum.data.discs.flatMap { it.tracksIds },
                        currentTrackPercent = 0f,
                        currentTrackIndex = 0,
                        progressSec = 0,
                    )
                    mutableIsPlaying.value = true
                    logger.info("Loaded album $albumId")
                }
            }
        }
    }

    override fun loadUserPlaylist(userPlaylistId: String) {
        TODO("Not yet implemented")
    }

    override fun loadTrack(trackId: String) {
        TODO("Not yet implemented")
    }

    override fun togglePlayPause() {
        TODO("Not yet implemented")
    }

    override fun seekToPercentage(percentage: Float) {
        TODO("Not yet implemented")
    }

    override fun setIsPlaying(isPlaying: Boolean) {
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

    override fun loadTrackIndex(index: Int) {
        TODO("Not yet implemented")
    }

    override fun goToPreviousPlaylist() {
        TODO("Not yet implemented")
    }

    override fun goToNextPlaylist() {
        TODO("Not yet implemented")
    }

    override fun moveTrack(fromIndex: Int, toIndex: Int) {
        TODO("Not yet implemented")
    }

    override fun addTracksToPlaylist(tracksIds: List<String>) {
        TODO("Not yet implemented")
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        TODO("Not yet implemented")
    }

    override fun skipToNextTrack() {
        TODO("Not yet implemented")
    }

    override fun skipToPreviousTrack() {
        TODO("Not yet implemented")
    }
}