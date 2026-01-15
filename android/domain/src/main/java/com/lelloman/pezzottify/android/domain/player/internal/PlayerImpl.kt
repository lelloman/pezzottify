package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackStateStore
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
import kotlinx.coroutines.flow.combine
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
    private val playbackStateStore: PlaybackStateStore,
    private val coroutineScope: CoroutineScope = GlobalScope,
) : PezzottifyPlayer, ControlsAndStatePlayer by platformPlayer {

    private val logger by loggerFactory

    private var loadNexPlaylistJob: Job? = null
    private var statePersistenceJob: Job? = null
    private var restorationAttempted = false

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

        // Persist state periodically and when playback state changes
        statePersistenceJob?.cancel()
        statePersistenceJob = coroutineScope.launch(Dispatchers.Main) {
            combine(
                playbackPlaylist,
                platformPlayer.isPlaying,
                platformPlayer.currentTrackIndex,
                platformPlayer.currentTrackProgressSec,
            ) { playlist, isPlaying, trackIndex, progressSec ->
                SaveStateData(playlist, isPlaying, trackIndex, progressSec)
            }.collect { data ->
                val playlist = data.playlist ?: return@collect
                val trackIndex = data.trackIndex ?: return@collect
                val progressSec = data.progressSec ?: 0

                // Save state when we have valid playback info
                playbackStateStore.saveState(
                    playlist = playlist,
                    currentTrackIndex = trackIndex,
                    positionMs = progressSec * 1000L,
                    isPlaying = data.isPlaying,
                )
            }
        }
    }

    private data class SaveStateData(
        val playlist: PlaybackPlaylist?,
        val isPlaying: Boolean,
        val trackIndex: Int?,
        val progressSec: Int?,
    )

    /**
     * Attempts to restore a previously saved playback state.
     * Called automatically when the user tries to play but the service is not active.
     *
     * @return true if restoration was attempted, false if no saved state exists
     */
    override suspend fun tryRestoreState(): Boolean {
        if (restorationAttempted) return false

        val savedState = playbackStateStore.loadState() ?: return false
        restorationAttempted = true

        logger.info("Restoring saved playback state: ${savedState.playlist.tracksIds.size} tracks, index=${savedState.currentTrackIndex}")

        // Restore the playlist
        mutablePlaybackPlaylist.value = savedState.playlist

        // Load tracks into player
        val baseUrl = configStore.baseUrl.value
        val urls = savedState.playlist.tracksIds.map { "$baseUrl/v1/content/stream/$it" }
        platformPlayer.loadPlaylist(urls)

        // Seek to saved position
        platformPlayer.loadTrackIndex(savedState.currentTrackIndex)

        // Clear saved state after successful restoration
        playbackStateStore.clearState()

        return true
    }

    /**
     * Override togglePlayPause to attempt state restoration if player is inactive.
     */
    override fun togglePlayPause() {
        if (!platformPlayer.isActive.value) {
            // Player is not active - try to restore saved state
            runOnPlayerThread {
                val restored = tryRestoreState()
                if (!restored) {
                    // No saved state to restore, just forward to platform player
                    // (will likely do nothing, but that's expected)
                    platformPlayer.togglePlayPause()
                }
                // If restored, playback starts automatically via loadPlaylist
            }
        } else {
            platformPlayer.togglePlayPause()
        }
    }

    /**
     * Override setIsPlaying to attempt state restoration if player is inactive.
     */
    override fun setIsPlaying(isPlaying: Boolean) {
        if (isPlaying && !platformPlayer.isActive.value) {
            // Trying to play but player is not active - try to restore saved state
            runOnPlayerThread {
                val restored = tryRestoreState()
                if (!restored) {
                    // No saved state to restore, just forward to platform player
                    platformPlayer.setIsPlaying(isPlaying)
                }
                // If restored, playback starts automatically via loadPlaylist
            }
        } else {
            platformPlayer.setIsPlaying(isPlaying)
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

    override fun loadUserPlaylist(userPlaylistId: String, startTrackId: String?) {
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

                    // If a specific track was requested, start from that track
                    if (startTrackId != null) {
                        val startIndex = tracksIds.indexOf(startTrackId)
                        if (startIndex >= 0) {
                            platformPlayer.loadTrackIndex(startIndex)
                            logger.info("Starting user playlist $userPlaylistId at track index $startIndex (trackId: $startTrackId)")
                        } else {
                            logger.warn("Start track $startTrackId not found in user playlist $userPlaylistId, starting from beginning")
                        }
                    }

                    logger.info("Loaded user playlist $userPlaylistId with ${tracksIds.size} tracks")
                } else {
                    logger.warn("User playlist $userPlaylistId not found or empty")
                }
            }
        }
    }

    override fun addUserPlaylistToQueue(userPlaylistId: String) {
        runOnPlayerThread {
            val playlist = withTimeoutOrNull(2.seconds) {
                userPlaylistStore.getPlaylist(userPlaylistId).first()
            }
            if (playlist != null && playlist.trackIds.isNotEmpty()) {
                addTracksToPlaylist(playlist.trackIds)
                logger.info("Added user playlist $userPlaylistId (${playlist.trackIds.size} tracks) to queue")
            } else {
                logger.warn("User playlist $userPlaylistId not found or empty")
            }
        }
    }

    override fun loadSingleTrack(trackId: String) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = PlaybackPlaylistContext.UserMix,
                    tracksIds = listOf(trackId),
                )
                platformPlayer.setIsPlaying(true)
                val baseUrl = configStore.baseUrl.value
                val url = "$baseUrl/v1/content/stream/$trackId"
                platformPlayer.loadPlaylist(listOf(url))
                logger.info("Loaded single track $trackId")
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
        // Clear persisted state on logout so we don't restore stale session
        coroutineScope.launch {
            playbackStateStore.clearState()
        }
        restorationAttempted = false
        logger.info("Cleared player session")
    }

    override fun toggleShuffle() {
        platformPlayer.toggleShuffle()
    }

    override fun cycleRepeatMode() {
        platformPlayer.cycleRepeatMode()
    }

    override fun retry() {
        platformPlayer.retry()
    }
}