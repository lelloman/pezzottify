package com.lelloman.pezzottify.android.domain.player

import kotlinx.coroutines.flow.StateFlow

interface PlaybackMetadataProvider {
    val queueState: StateFlow<PlaybackQueueState?>
}
