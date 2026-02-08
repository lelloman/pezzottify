package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Switches between local and remote PlaybackMetadataProvider based on PlaybackModeManager.mode.
 */
@OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
@Singleton
class CompositePlaybackMetadataProvider internal constructor(
    private val localProvider: PlaybackMetadataProviderImpl,
    private val remoteProvider: RemotePlaybackMetadataProvider,
    private val playbackModeManager: PlaybackModeManager,
    private val scope: CoroutineScope,
) : PlaybackMetadataProvider {

    @Inject
    constructor(
        localProvider: PlaybackMetadataProviderImpl,
        remoteProvider: RemotePlaybackMetadataProvider,
        playbackModeManager: PlaybackModeManager,
    ) : this(localProvider, remoteProvider, playbackModeManager, GlobalScope)

    private val _queueState = MutableStateFlow<PlaybackQueueState?>(null)
    override val queueState: StateFlow<PlaybackQueueState?> = _queueState.asStateFlow()

    init {
        scope.launch {
            playbackModeManager.mode.flatMapLatest { mode ->
                when (mode) {
                    is PlaybackMode.Local -> localProvider.queueState
                    is PlaybackMode.Remote -> remoteProvider.queueState
                }
            }.collect { _queueState.value = it }
        }
    }
}
