package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
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
        val currentIndex = platformPlayer.currentTrackIndex.value ?: 0

        // First emit a state with empty metadata but correct structure
        // This allows UI to show loading state
        mutableQueueState.value = PlaybackQueueState(
            tracks = emptyList(),
            currentIndex = currentIndex,
        )

        // Collect all unique album and artist IDs we need to fetch
        val trackDataMap = mutableMapOf<String, Track>()
        val albumIds = mutableSetOf<String>()
        val artistIds = mutableSetOf<String>()

        // Fetch all tracks first
        for (trackId in tracksIds) {
            val track = fetchTrack(trackId)
            if (track != null) {
                trackDataMap[trackId] = track
                albumIds.add(track.albumId)
                artistIds.addAll(track.artistsIds)
            }
        }

        // Fetch all albums
        val albumDataMap = mutableMapOf<String, Album>()
        for (albumId in albumIds) {
            val album = fetchAlbum(albumId)
            if (album != null) {
                albumDataMap[albumId] = album
            }
        }

        // Fetch all artists
        val artistDataMap = mutableMapOf<String, Artist>()
        for (artistId in artistIds) {
            val artist = fetchArtist(artistId)
            if (artist != null) {
                artistDataMap[artistId] = artist
            }
        }

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
                albumId = track.albumId,
                albumName = album?.name ?: "",
                artworkUrl = artworkUrl,
                durationSeconds = track.durationSeconds,
            )
        }

        val updatedIndex = platformPlayer.currentTrackIndex.value ?: 0
        mutableQueueState.value = PlaybackQueueState(
            tracks = tracksMetadata,
            currentIndex = updatedIndex,
        )

        logger.info("Loaded metadata for ${tracksMetadata.size}/${tracksIds.size} tracks")
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
