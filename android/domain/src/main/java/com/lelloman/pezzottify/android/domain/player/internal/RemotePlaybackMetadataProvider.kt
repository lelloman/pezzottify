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
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Provides playback metadata for the currently controlled remote device.
 * Derives track metadata from the remote device's state broadcasts.
 */
@OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
@Singleton
class RemotePlaybackMetadataProvider internal constructor(
    private val playbackSessionHandler: PlaybackSessionHandler,
    private val playbackModeManager: PlaybackModeManager,
    private val configStore: ConfigStore,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : PlaybackMetadataProvider {

    @Inject
    constructor(
        playbackSessionHandler: PlaybackSessionHandler,
        playbackModeManager: PlaybackModeManager,
        configStore: ConfigStore,
        loggerFactory: LoggerFactory,
    ) : this(playbackSessionHandler, playbackModeManager, configStore, GlobalScope, loggerFactory)

    private val logger = loggerFactory.getLogger(RemotePlaybackMetadataProvider::class)

    private val _queueState = MutableStateFlow<PlaybackQueueState?>(null)
    override val queueState: StateFlow<PlaybackQueueState?> = _queueState.asStateFlow()

    init {
        scope.launch {
            playbackModeManager.mode.flatMapLatest { mode ->
                when (mode) {
                    is PlaybackMode.Remote -> playbackSessionHandler.otherDeviceStates
                        .map { it[mode.deviceId]?.toQueueState() }
                    is PlaybackMode.Local -> flowOf(null)
                }
            }.collect { _queueState.value = it }
        }
    }

    private fun RemotePlaybackState.toQueueState(): PlaybackQueueState? {
        val track = currentTrack ?: return null
        val baseUrl = configStore.baseUrl.value.trimEnd('/')

        val trackMetadata = TrackMetadata(
            trackId = track.id,
            trackName = track.title,
            artistNames = listOfNotNull(track.artistName),
            albumId = "",
            albumName = track.albumTitle ?: "",
            artworkUrl = track.imageId?.let { "$baseUrl/v1/content/image/$it" },
            imageId = track.imageId,
            durationSeconds = (track.durationMs / 1000).toInt(),
        )

        return PlaybackQueueState(
            tracks = listOf(trackMetadata),
            currentIndex = 0,
            loadingState = QueueLoadingState.LOADED,
        )
    }
}
