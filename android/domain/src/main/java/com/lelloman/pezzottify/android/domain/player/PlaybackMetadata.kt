package com.lelloman.pezzottify.android.domain.player

data class TrackMetadata(
    val trackId: String,
    val trackName: String,
    val artistNames: List<String>,
    val albumId: String,
    val albumName: String,
    val artworkUrl: String?,
    val durationSeconds: Int,
)

data class PlaybackQueueState(
    val tracks: List<TrackMetadata>,
    val currentIndex: Int,
) {
    val currentTrack: TrackMetadata?
        get() = tracks.getOrNull(currentIndex)
}
