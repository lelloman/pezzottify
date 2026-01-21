package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.statics.TrackAvailability

data class TrackMetadata(
    val trackId: String,
    val trackName: String,
    val artistNames: List<String>,
    val albumId: String,
    val albumName: String,
    val artworkUrl: String?,
    val durationSeconds: Int,
    val availability: TrackAvailability = TrackAvailability.Available,
)

enum class QueueLoadingState {
    LOADED,
    LOADING,
}

data class PlaybackQueueState(
    val tracks: List<TrackMetadata>,
    val currentIndex: Int,
    val loadingState: QueueLoadingState = QueueLoadingState.LOADED,
) {
    val currentTrack: TrackMetadata?
        get() = tracks.getOrNull(currentIndex)

    val isLoading: Boolean
        get() = loadingState == QueueLoadingState.LOADING
}
