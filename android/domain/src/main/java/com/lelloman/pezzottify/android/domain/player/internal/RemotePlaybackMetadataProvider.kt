package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState
import com.lelloman.pezzottify.android.domain.playbacksession.RemoteTrackInfo
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.seconds

/**
 * Provides playback metadata for the currently controlled remote device.
 * Derives track metadata from the remote device's state broadcasts and queue data.
 * Resolves track IDs from queue to full TrackMetadata via StaticsProvider.
 */
@OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
@Singleton
class RemotePlaybackMetadataProvider internal constructor(
    private val playbackSessionHandler: PlaybackSessionHandler,
    private val playbackModeManager: PlaybackModeManager,
    private val configStore: ConfigStore,
    private val staticsProvider: StaticsProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : PlaybackMetadataProvider {

    @Inject
    constructor(
        playbackSessionHandler: PlaybackSessionHandler,
        playbackModeManager: PlaybackModeManager,
        configStore: ConfigStore,
        staticsProvider: StaticsProvider,
        loggerFactory: LoggerFactory,
    ) : this(playbackSessionHandler, playbackModeManager, configStore, staticsProvider, GlobalScope, loggerFactory)

    private val logger = loggerFactory.getLogger(RemotePlaybackMetadataProvider::class)

    private val _queueState = MutableStateFlow<PlaybackQueueState?>(null)
    override val queueState: StateFlow<PlaybackQueueState?> = _queueState.asStateFlow()

    private var currentQueueTrackIds: List<String>? = null
    private var currentRemoteDeviceId: Int? = null
    private var metadataLoadJob: Job? = null

    init {
        scope.launch {
            playbackModeManager.mode.flatMapLatest { mode ->
                when (mode) {
                    is PlaybackMode.Remote -> combine(
                        playbackSessionHandler.otherDeviceStates,
                        playbackSessionHandler.otherDeviceQueues,
                    ) { states, queues ->
                        val state = states[mode.deviceId]
                        val queue = queues[mode.deviceId]
                        Triple(mode.deviceId, state, queue)
                    }
                    is PlaybackMode.Local -> flowOf(Triple(null, null, null))
                }
            }.collect { (deviceId, remoteState, queueTrackIds) ->
                if (deviceId != currentRemoteDeviceId) {
                    currentRemoteDeviceId = deviceId
                    currentQueueTrackIds = null
                    metadataLoadJob?.cancel()
                    metadataLoadJob = null
                }

                if (remoteState == null && queueTrackIds == null) {
                    _queueState.value = null
                    currentQueueTrackIds = null
                    metadataLoadJob?.cancel()
                    return@collect
                }

                if (queueTrackIds != null && queueTrackIds != currentQueueTrackIds) {
                    // Queue changed - resolve full metadata
                    currentQueueTrackIds = queueTrackIds
                    metadataLoadJob?.cancel()

                    // Emit loading state with current track immediately
                    _queueState.value = remoteState?.let {
                        buildQueueFromCurrentTrackOnly(it)?.copy(loadingState = QueueLoadingState.LOADING)
                    }

                    metadataLoadJob = scope.launch {
                        resolveQueueMetadata(
                            queueTrackIds,
                            remoteState ?: RemotePlaybackState(
                                currentTrack = null,
                                position = 0.0,
                                isPlaying = false,
                                volume = 1.0f,
                                muted = false,
                                shuffle = false,
                                repeat = "off",
                                timestamp = 0L,
                                queuePosition = 0,
                            ),
                        )
                    }
                } else if (queueTrackIds != null && queueTrackIds == currentQueueTrackIds && remoteState != null) {
                    // Same queue, just update currentIndex from state
                    val current = _queueState.value
                    if (current != null && current.loadingState == QueueLoadingState.LOADED) {
                        _queueState.value = current.copy(currentIndex = remoteState.queuePosition)
                    }
                } else if (remoteState != null) {
                    // No queue data - fall back to current track only
                    _queueState.value = buildQueueFromCurrentTrackOnly(remoteState)
                }
            }
        }
    }

    private fun buildQueueFromCurrentTrackOnly(state: RemotePlaybackState): PlaybackQueueState? {
        val track = state.currentTrack ?: return null
        return PlaybackQueueState(
            tracks = listOf(trackInfoToMetadata(track)),
            currentIndex = 0,
            loadingState = QueueLoadingState.LOADED,
        )
    }

    private fun trackInfoToMetadata(track: RemoteTrackInfo): TrackMetadata {
        val baseUrl = configStore.baseUrl.value.trimEnd('/')
        return TrackMetadata(
            trackId = track.id,
            trackName = track.title,
            artistNames = listOfNotNull(track.artistName),
            albumId = "",
            albumName = track.albumTitle ?: "",
            artworkUrl = track.imageId?.let { "$baseUrl/v1/content/image/$it" },
            imageId = track.imageId,
            durationSeconds = (track.durationMs / 1000).toInt(),
        )
    }

    private suspend fun resolveQueueMetadata(trackIds: List<String>, state: RemotePlaybackState) {
        logger.debug("Resolving metadata for ${trackIds.size} queue tracks")

        val baseUrl = configStore.baseUrl.value.trimEnd('/')

        // Fetch all tracks in parallel
        val trackResults = trackIds.map { trackId ->
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
            .mapNotNull { (id, data) -> data?.let { id to data } }
            .toMap()
        val artistDataMap = artistDeferred.awaitAll()
            .mapNotNull { (id, data) -> data?.let { id to data } }
            .toMap()

        // Build metadata for each track, falling back to current track info for unresolved tracks
        val currentTrackInfo = state.currentTrack
        val tracksMetadata = trackIds.map { trackId ->
            val track = trackDataMap[trackId]
            if (track != null) {
                val album = albumDataMap[track.albumId]
                val artistNames = track.artistsIds.mapNotNull { artistDataMap[it]?.name }
                TrackMetadata(
                    trackId = trackId,
                    trackName = track.name,
                    artistNames = artistNames,
                    primaryArtistId = track.artistsIds.firstOrNull() ?: "",
                    albumId = track.albumId,
                    albumName = album?.name ?: "",
                    artworkUrl = album?.displayImageId?.let { "$baseUrl/v1/content/image/$it" },
                    imageId = album?.displayImageId,
                    durationSeconds = track.durationSeconds,
                    availability = track.availability,
                )
            } else if (currentTrackInfo != null && currentTrackInfo.id == trackId) {
                // Use the current track info from state as fallback
                trackInfoToMetadata(currentTrackInfo)
            } else {
                // Minimal fallback for completely unresolved tracks
                TrackMetadata(
                    trackId = trackId,
                    trackName = trackId,
                    artistNames = emptyList(),
                    albumId = "",
                    albumName = "",
                    artworkUrl = null,
                    durationSeconds = 0,
                )
            }
        }

        _queueState.value = PlaybackQueueState(
            tracks = tracksMetadata,
            currentIndex = state.queuePosition,
            loadingState = QueueLoadingState.LOADED,
        )

        logger.info("Resolved metadata for ${tracksMetadata.size} queue tracks, currentIndex=${state.queuePosition}")
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
