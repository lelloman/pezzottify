package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState
import com.lelloman.pezzottify.android.domain.playbacksession.RemoteTrackInfo
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Provides playback metadata for the currently controlled remote device.
 * Derives track metadata from the remote device's state broadcasts and queue data.
 * Resolves track IDs from queue to full TrackMetadata via StaticsProvider.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@Singleton
class RemotePlaybackMetadataProvider @Inject constructor(
    private val playbackSessionHandler: PlaybackSessionHandler,
    private val playbackModeManager: PlaybackModeManager,
    private val configStore: ConfigStore,
    private val staticsProvider: StaticsProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : PlaybackMetadataProvider {

    private val logger = loggerFactory.getLogger(RemotePlaybackMetadataProvider::class)

    private val _queueState = MutableStateFlow<PlaybackQueueState?>(null)
    override val queueState: StateFlow<PlaybackQueueState?> = _queueState.asStateFlow()

    private var currentQueueTrackIds: List<String>? = null
    private var currentQueueContext: PlaybackPlaylistContext? = null
    private var currentRemoteDeviceId: Int? = null
    private var metadataLoadJob: Job? = null

    @Volatile
    private var latestRemoteState: RemotePlaybackState? = null

    private val queueMetadataResolver = QueueMetadataResolver(staticsProvider, configStore)

    init {
        scope.launch {
            playbackModeManager.mode.flatMapLatest { mode ->
                when (mode) {
                    is PlaybackMode.Remote -> combine(
                        playbackSessionHandler.otherDeviceStates,
                        playbackSessionHandler.otherDeviceQueues,
                        playbackSessionHandler.otherDeviceQueueContexts,
                    ) { states, queues, contexts ->
                        val state = states[mode.deviceId]
                        val queue = queues[mode.deviceId]
                        RemoteQueueSnapshot(mode.deviceId, state, queue, contexts[mode.deviceId])
                    }
                    is PlaybackMode.Local -> flowOf(RemoteQueueSnapshot(null, null, null, null))
                }
            }.collect { (deviceId, remoteState, queueTrackIds, queueContext) ->
                latestRemoteState = remoteState
                if (deviceId != currentRemoteDeviceId) {
                    currentRemoteDeviceId = deviceId
                    currentQueueTrackIds = null
                    currentQueueContext = null
                    metadataLoadJob?.cancel()
                    metadataLoadJob = null
                }

                if (remoteState == null && queueTrackIds == null) {
                    _queueState.value = null
                    currentQueueTrackIds = null
                    currentQueueContext = null
                    metadataLoadJob?.cancel()
                    return@collect
                }

                if (queueTrackIds != null && (queueTrackIds != currentQueueTrackIds || queueContext != currentQueueContext)) {
                    // Queue changed - resolve full metadata
                    currentQueueTrackIds = queueTrackIds
                    currentQueueContext = queueContext
                    metadataLoadJob?.cancel()

                    // Emit loading state with current track immediately
                    _queueState.value = remoteState?.let {
                        buildQueueFromCurrentTrackOnly(it, queueContext)
                            ?.copy(loadingState = QueueLoadingState.LOADING)
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
                            queueContext,
                        )
                    }
                } else if (queueTrackIds != null && queueTrackIds == currentQueueTrackIds && remoteState != null) {
                    // Same queue, just update currentIndex from state
                    val current = _queueState.value
                    if (current != null && current.loadingState == QueueLoadingState.LOADED) {
                        _queueState.value = current.copy(
                            currentIndex = remoteState.queuePosition,
                            context = queueContext,
                        )
                    }
                } else if (remoteState != null) {
                    // No queue data - fall back to current track only
                    _queueState.value = buildQueueFromCurrentTrackOnly(remoteState, queueContext)
                }
            }
        }
    }

    private fun buildQueueFromCurrentTrackOnly(
        state: RemotePlaybackState,
        context: PlaybackPlaylistContext?,
    ): PlaybackQueueState? {
        val track = state.currentTrack ?: return null
        return PlaybackQueueState(
            tracks = listOf(trackInfoToMetadata(track)),
            currentIndex = 0,
            loadingState = QueueLoadingState.LOADED,
            context = context,
        )
    }

    private fun trackInfoToMetadata(track: RemoteTrackInfo): TrackMetadata {
        val baseUrl = configStore.baseUrl.value.trimEnd('/')
        return TrackMetadata(
            trackId = track.id,
            trackName = track.title,
            artistNames = listOfNotNull(track.artistName),
            primaryArtistId = track.artistId ?: track.artistsIds.firstOrNull() ?: "",
            artistIds = track.artistsIds.ifEmpty { listOfNotNull(track.artistId) },
            albumId = track.albumId ?: "",
            albumName = track.albumTitle ?: "",
            artworkUrl = track.imageId?.let { "$baseUrl/v1/content/image/$it" },
            imageId = track.imageId,
            durationSeconds = (track.durationMs / 1000).toInt(),
        )
    }

    private suspend fun resolveQueueMetadata(
        trackIds: List<String>,
        state: RemotePlaybackState,
        context: PlaybackPlaylistContext?,
    ) {
        logger.debug("Resolving metadata for ${trackIds.size} queue tracks")

        // Re-emitted as tracks/albums/artists load, so items that miss the first pass
        // (cold cache) are patched in later instead of staying placeholders.
        queueMetadataResolver.observe(trackIds).collect { resolved ->
            // The remote state keeps moving while metadata resolves; use the latest one.
            val latestState = latestRemoteState ?: state
            val currentTrackInfo = latestState.currentTrack
            val tracksMetadata = resolved.tracks.mapIndexed { index, metadata ->
                val trackId = trackIds[index]
                when {
                    metadata != null -> metadata
                    // Use the current track info from state as fallback
                    currentTrackInfo != null && currentTrackInfo.id == trackId ->
                        trackInfoToMetadata(currentTrackInfo)
                    else -> QueueMetadataResolver.placeholder(trackId)
                }
            }

            _queueState.value = PlaybackQueueState(
                tracks = tracksMetadata,
                currentIndex = latestState.queuePosition,
                loadingState = QueueLoadingState.LOADED,
                context = context,
            )

            logger.info(
                "Resolved metadata for ${resolved.tracks.count { it != null }}/${trackIds.size} " +
                    "queue tracks, currentIndex=${latestState.queuePosition}"
            )
        }
    }

    private data class RemoteQueueSnapshot(
        val deviceId: Int?,
        val state: RemotePlaybackState?,
        val queue: List<String>?,
        val context: PlaybackPlaylistContext?,
    )
}
