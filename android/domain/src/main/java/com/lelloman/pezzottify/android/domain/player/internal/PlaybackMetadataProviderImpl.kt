package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class PlaybackMetadataProviderImpl internal constructor(
    private val player: PezzottifyPlayer,
    private val platformPlayer: PlatformPlayer,
    private val staticsProvider: StaticsProvider,
    private val configStore: ConfigStore,
    loggerFactory: LoggerFactory,
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
    private val queueMetadataResolver: QueueMetadataResolver =
        QueueMetadataResolver(staticsProvider, configStore),
) : PlaybackMetadataProvider {

    private val logger by loggerFactory

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
                // with a loading indicator while metadata is being fetched. The resolver
                // keeps observing items that are not loaded yet and the queue state is
                // re-emitted as they arrive, so a slow cold-cache fetch heals itself.
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
        queueMetadataResolver.observe(tracksIds).collect { resolved ->
            val tracksMetadata = resolved.tracks.mapIndexed { index, metadata ->
                metadata ?: QueueMetadataResolver.placeholder(tracksIds[index])
            }
            val updatedIndex = platformPlayer.currentTrackIndex.value
                ?: mutableQueueState.value?.currentIndex
                ?: 0
            mutableQueueState.value = PlaybackQueueState(
                tracks = tracksMetadata,
                currentIndex = updatedIndex,
                loadingState = QueueLoadingState.LOADED,
            )
            val resolvedCount = resolved.tracks.count { it != null }
            if (resolved.isComplete) {
                logger.info("Loaded metadata for ${tracksIds.size} tracks, currentIndex=$updatedIndex")
            } else {
                logger.info("Partial metadata for $resolvedCount/${tracksIds.size} tracks, waiting for the rest")
            }
        }
    }
}
