package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.time.Duration.Companion.seconds

@OptIn(DelicateCoroutinesApi::class)
internal class PlayerImpl(
    private val staticsProvider: StaticsProvider,
    loggerFactory: LoggerFactory,
    private val platformPlayer: PlatformPlayer,
    private val configStore: ConfigStore,
    private val coroutineScope: CoroutineScope = GlobalScope,
) : PezzottifyPlayer, ControlsAndStatePlayer by platformPlayer {

    private val logger by loggerFactory

    private var loadNexPlaylistJob: Job? = null

    private val mutablePlaybackPlaylist = MutableStateFlow<PlaybackPlaylist?>(null)
    override val playbackPlaylist = mutablePlaybackPlaylist.asStateFlow()

    override val canGoToPreviousPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")

    override val canGoToNextPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")

    private fun runOnPlayerThread(block: suspend () -> Unit) =
        coroutineScope.launch(Dispatchers.Main) {
            block()
        }

    override fun initialize() {

        runOnPlayerThread {
            platformPlayer.isActive.collect { isActive ->
                if (!isActive) {
                    mutablePlaybackPlaylist.value = null
                }
            }
        }
    }

    override fun loadAlbum(albumId: String, startTrackId: String?) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                val loadedAlbum = withTimeoutOrNull(2.seconds) {
                    staticsProvider.provideAlbum(albumId)
                        .filterIsInstance<StaticsItem.Loaded<Album>>()
                        .first()
                }
                if (loadedAlbum != null) {
                    val tracksIds = loadedAlbum.data.discs.flatMap { it.tracksIds }
                    mutablePlaybackPlaylist.value = PlaybackPlaylist(
                        context = PlaybackPlaylistContext.Album(albumId),
                        tracksIds = tracksIds,
                    )
                    platformPlayer.setIsPlaying(true)
                    logger.info("Loading new track list into platform player.")
                    val baseUrl = configStore.baseUrl.value
                    val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                    platformPlayer.loadPlaylist(urls)

                    // If a specific track was requested, start from that track
                    if (startTrackId != null) {
                        val startIndex = tracksIds.indexOf(startTrackId)
                        if (startIndex >= 0) {
                            platformPlayer.loadTrackIndex(startIndex)
                            logger.info("Starting album $albumId at track index $startIndex (trackId: $startTrackId)")
                        } else {
                            logger.warn("Start track $startTrackId not found in album $albumId, starting from beginning")
                        }
                    }

                    logger.info("Loaded album $albumId")
                }
            }
        }
    }

    override fun loadUserPlaylist(userPlaylistId: String) {
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
}