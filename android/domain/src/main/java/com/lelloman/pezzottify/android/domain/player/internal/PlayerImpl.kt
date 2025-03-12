package com.lelloman.pezzottify.android.domain.player.internal

import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
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
import kotlinx.coroutines.android.asCoroutineDispatcher
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.CoroutineContext
import kotlin.time.Duration.Companion.seconds

@OptIn(DelicateCoroutinesApi::class)
internal class PlayerImpl(
    private val staticsProvider: StaticsProvider,
    loggerFactory: LoggerFactory,
    private val platformPlayerFactory: PlatformPlayer.Factory,
    private val configStore: ConfigStore,
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

    private val mutableUserSoughtTrackPercent = MutableSharedFlow<Float>()
    private val coroutineContext: CoroutineContext
    private val looper: Looper

    init {
        val handlerThread = HandlerThread("MyThread")
        handlerThread.start()
        val handler = Handler(handlerThread.looper)
        coroutineContext = handler.asCoroutineDispatcher()
        looper = handler.looper
    }

    private fun runOnPlayerThread(block: suspend () -> Unit) =
        coroutineScope.launch(coroutineContext) {
            block()
        }

    override fun initialize() {
        runOnPlayerThread {
            val platformPlayer = platformPlayerFactory.create(looper)

            runOnPlayerThread { isPlaying.collect { platformPlayer.setIsPlaying(it) } }
            runOnPlayerThread {
                playbackPlaylist
                    .map { it?.tracksIds }
                    .filterNotNull()
                    .distinctUntilChanged()
                    .collect { trackId ->
                        logger.info("Loading new track list into platform player.")
                        val baseUrl = configStore.baseUrl.value
                        val urls = trackId.map { "$baseUrl/v1/content/stream/$it" }
                        platformPlayer.loadPlaylist(urls)
                    }
            }
            runOnPlayerThread {
                playbackPlaylist
                    .map { it?.currentTrackIndex }
                    .filterNotNull()
                    .distinctUntilChanged()
                    .collect {
                        platformPlayer.loadTrackIndex(it)
                    }
            }
            runOnPlayerThread {
                mutableUserSoughtTrackPercent.collect { platformPlayer.seekTrackProgressPercent(it) }
            }
        }
    }

    override fun loadAlbum(albumId: String) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
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
        mutableIsPlaying.value = mutableIsPlaying.value.not()
    }

    override fun seekToPercentage(percentage: Float) {
        runOnPlayerThread {
            mutableUserSoughtTrackPercent.emit(percentage)
        }
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
        val playbackPlaylist = playbackPlaylist.value ?: return
        val currentIndex = playbackPlaylist.currentTrackIndex ?: return
        if (currentIndex == playbackPlaylist.tracksIds.lastIndex) {
            return
        }
        mutablePlaybackPlaylist.value = playbackPlaylist.copy(currentTrackIndex = currentIndex + 1)
    }

    override fun skipToPreviousTrack() {
        val playbackPlaylist = playbackPlaylist.value ?: return
        val currentIndex = playbackPlaylist.currentTrackIndex ?: return
        if (currentIndex == 0) {
            return
        }
        mutablePlaybackPlaylist.value = playbackPlaylist.copy(currentTrackIndex = currentIndex - 1)
    }
}