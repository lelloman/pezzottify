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
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
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
    private val userPlaylistStore: UserPlaylistStore,
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

    override fun addAlbumToPlaylist(albumId: String) {
        runOnPlayerThread {
            val loadedAlbum = withTimeoutOrNull(2.seconds) {
                staticsProvider.provideAlbum(albumId)
                    .filterIsInstance<StaticsItem.Loaded<Album>>()
                    .first()
            }
            if (loadedAlbum != null) {
                val tracksIds = loadedAlbum.data.discs.flatMap { it.tracksIds }
                addTracksToPlaylist(tracksIds)
                logger.info("Added album $albumId (${tracksIds.size} tracks) to playlist")
            }
        }
    }

    override fun loadUserPlaylist(userPlaylistId: String) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                val playlist = withTimeoutOrNull(2.seconds) {
                    userPlaylistStore.getPlaylist(userPlaylistId).first()
                }
                if (playlist != null && playlist.trackIds.isNotEmpty()) {
                    val tracksIds = playlist.trackIds
                    mutablePlaybackPlaylist.value = PlaybackPlaylist(
                        context = PlaybackPlaylistContext.UserPlaylist(userPlaylistId, isEdited = false),
                        tracksIds = tracksIds,
                    )
                    platformPlayer.setIsPlaying(true)
                    logger.info("Loading user playlist into platform player.")
                    val baseUrl = configStore.baseUrl.value
                    val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                    platformPlayer.loadPlaylist(urls)
                    logger.info("Loaded user playlist $userPlaylistId with ${tracksIds.size} tracks")
                } else {
                    logger.warn("User playlist $userPlaylistId not found or empty")
                }
            }
        }
    }

    override fun forward10Sec() {
        platformPlayer.forward10Sec( )
    }

    override fun rewind10Sec() {
        platformPlayer.rewind10Sec()
    }

    override fun stop() {
        platformPlayer.stop()
    }

    override fun setVolume(volume: Float) {
        platformPlayer.setVolume(volume)
    }

    override fun setMuted(isMuted: Boolean) {
        platformPlayer.setMuted(isMuted)
    }

    override fun loadTrackIndex(index: Int) {
        platformPlayer.loadTrackIndex(index)
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
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value
            if (currentPlaylist != null) {
                // Add tracks to the existing playlist
                val newTracksIds = currentPlaylist.tracksIds + tracksIds
                val newContext = when (val ctx = currentPlaylist.context) {
                    is PlaybackPlaylistContext.Album -> PlaybackPlaylistContext.UserMix
                    is PlaybackPlaylistContext.UserPlaylist -> ctx.copy(isEdited = true)
                    is PlaybackPlaylistContext.UserMix -> ctx
                }
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = newContext,
                    tracksIds = newTracksIds,
                )
                // Add new tracks to platform player
                val baseUrl = configStore.baseUrl.value
                val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                platformPlayer.addMediaItems(urls)
                logger.info("Added ${tracksIds.size} tracks to playlist")
            } else {
                // No playlist exists, create a new UserMix playlist
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = PlaybackPlaylistContext.UserMix,
                    tracksIds = tracksIds,
                )
                platformPlayer.setIsPlaying(true)
                val baseUrl = configStore.baseUrl.value
                val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                platformPlayer.loadPlaylist(urls)
                logger.info("Created new UserMix playlist with ${tracksIds.size} tracks")
            }
        }
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value ?: return@runOnPlayerThread
            val trackIndex = currentPlaylist.tracksIds.indexOf(trackId)
            if (trackIndex < 0) {
                logger.warn("Track $trackId not found in playlist, cannot remove")
                return@runOnPlayerThread
            }

            val newTracksIds = currentPlaylist.tracksIds.toMutableList().apply {
                removeAt(trackIndex)
            }

            // Update context to reflect the playlist has been modified
            val newContext = when (val ctx = currentPlaylist.context) {
                is PlaybackPlaylistContext.Album -> PlaybackPlaylistContext.UserMix
                is PlaybackPlaylistContext.UserPlaylist -> ctx.copy(isEdited = true)
                is PlaybackPlaylistContext.UserMix -> ctx
            }

            mutablePlaybackPlaylist.value = PlaybackPlaylist(
                context = newContext,
                tracksIds = newTracksIds,
            )

            // Remove from platform player
            platformPlayer.removeMediaItem(trackIndex)
            logger.info("Removed track $trackId (index $trackIndex) from playlist")
        }
    }

    override fun clearSession() {
        loadNexPlaylistJob?.cancel()
        loadNexPlaylistJob = null
        mutablePlaybackPlaylist.value = null
        platformPlayer.clearSession()
        logger.info("Cleared player session")
    }

    override fun toggleShuffle() {
        platformPlayer.toggleShuffle()
    }

    override fun cycleRepeatMode() {
        platformPlayer.cycleRepeatMode()
    }
}