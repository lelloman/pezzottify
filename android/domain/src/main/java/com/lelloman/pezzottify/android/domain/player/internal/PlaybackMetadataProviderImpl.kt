package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.time.Duration.Companion.seconds

internal class PlaybackMetadataProviderImpl(
    private val player: PezzottifyPlayer,
    private val platformPlayer: PlatformPlayer,
    private val staticsProvider: StaticsProvider,
    private val configStore: ConfigStore,
    loggerFactory: LoggerFactory,
) : PlaybackMetadataProvider {

    private val logger by loggerFactory

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    private val mutableQueueState = MutableStateFlow<PlaybackQueueState?>(null)
    override val queueState: StateFlow<PlaybackQueueState?> = mutableQueueState.asStateFlow()

    private var metadataLoadJob: Job? = null
    private var currentPlaylistTracksIds: List<String>? = null

    init {
        observePlaylistChanges()
        observeTrackIndexChanges()
    }

    private fun observePlaylistChanges() {
        scope.launch {
            player.playbackPlaylist.collect { playlist ->
                if (playlist == null) {
                    mutableQueueState.value = null
                    currentPlaylistTracksIds = null
                    metadataLoadJob?.cancel()
                    logger.debug("Playlist cleared, resetting queue state")
                    return@collect
                }

                val tracksIds = playlist.tracksIds
                if (tracksIds == currentPlaylistTracksIds) {
                    logger.debug("Playlist tracks unchanged, skipping metadata reload")
                    return@collect
                }

                currentPlaylistTracksIds = tracksIds
                logger.debug("Playlist changed with ${tracksIds.size} tracks, loading metadata")

                // Cancel any ongoing metadata load
                metadataLoadJob?.cancel()

                // Emit loading state immediately so the UI can show the bottom player
                // with a loading indicator while metadata is being fetched
                val currentIndex = platformPlayer.currentTrackIndex.value ?: 0
                mutableQueueState.value = PlaybackQueueState(
                    tracks = emptyList(),
                    currentIndex = currentIndex,
                    loadingState = QueueLoadingState.LOADING,
                )

                metadataLoadJob = scope.launch {
                    loadMetadataForTracks(tracksIds)
                }
            }
        }
    }

    private fun observeTrackIndexChanges() {
        scope.launch {
            platformPlayer.currentTrackIndex.collect { index ->
                val currentState = mutableQueueState.value
                if (currentState != null && index != null && index != currentState.currentIndex) {
                    mutableQueueState.value = currentState.copy(currentIndex = index)
                    logger.debug("Track index changed to $index")
                }
            }
        }
    }

    private suspend fun loadMetadataForTracks(tracksIds: List<String>) {
        logger.debug("Loading metadata for ${tracksIds.size} tracks")

        // Fetch all tracks in parallel
        val trackResults = tracksIds.map { trackId ->
            scope.async { trackId to fetchTrack(trackId) }
        }.awaitAll()

        val trackDataMap = mutableMapOf<String, Track>()
        val albumIds = mutableSetOf<String>()
        val artistIds = mutableSetOf<String>()
        for ((trackId, track) in trackResults) {
            if (track != null) {
                trackDataMap[trackId] = track
                albumIds.add(track.albumId)
                artistIds.addAll(track.artistsIds)
            }
        }

        // Fetch all albums and artists in parallel
        val albumDeferred = albumIds.map { albumId ->
            scope.async { albumId to fetchAlbum(albumId) }
        }
        val artistDeferred = artistIds.map { artistId ->
            scope.async { artistId to fetchArtist(artistId) }
        }

        val albumDataMap = albumDeferred.awaitAll()
            .filter { it.second != null }
            .associate { it.first to it.second!! }
        val artistDataMap = artistDeferred.awaitAll()
            .filter { it.second != null }
            .associate { it.first to it.second!! }

        // Build metadata for each track
        val baseUrl = configStore.baseUrl.value.trimEnd('/')
        val tracksMetadata = tracksIds.mapNotNull { trackId ->
            val track = trackDataMap[trackId] ?: return@mapNotNull null
            val album = albumDataMap[track.albumId]
            val artistNames = track.artistsIds.mapNotNull { artistDataMap[it]?.name }

            val artworkUrl = album?.displayImageId?.let { imageId ->
                "$baseUrl/v1/content/image/$imageId"
            }

            TrackMetadata(
                trackId = trackId,
                trackName = track.name,
                artistNames = artistNames,
                primaryArtistId = track.artistsIds.firstOrNull() ?: "",
                albumId = track.albumId,
                albumName = album?.name ?: "",
                artworkUrl = artworkUrl,
                imageId = album?.displayImageId,
                durationSeconds = track.durationSeconds,
                availability = track.availability,
            )
        }

        val updatedIndex = platformPlayer.currentTrackIndex.value ?: 0
        mutableQueueState.value = PlaybackQueueState(
            tracks = tracksMetadata,
            currentIndex = updatedIndex,
            loadingState = QueueLoadingState.LOADED,
        )

        logger.info("Loaded metadata for ${tracksMetadata.size}/${tracksIds.size} tracks, currentIndex=$updatedIndex")
    }

    private suspend fun fetchTrack(trackId: String): Track? {
        return withTimeoutOrNull(2.seconds) {
            staticsProvider.provideTrack(trackId)
                .filterIsInstance<StaticsItem.Loaded<Track>>()
                .first()
                .data
        }
    }

    private suspend fun fetchAlbum(albumId: String): Album? {
        return withTimeoutOrNull(2.seconds) {
            staticsProvider.provideAlbum(albumId)
                .filterIsInstance<StaticsItem.Loaded<Album>>()
                .first()
                .data
        }
    }

    private suspend fun fetchArtist(artistId: String): Artist? {
        return withTimeoutOrNull(2.seconds) {
            staticsProvider.provideArtist(artistId)
                .filterIsInstance<StaticsItem.Loaded<Artist>>()
                .first()
                .data
        }
    }
}
