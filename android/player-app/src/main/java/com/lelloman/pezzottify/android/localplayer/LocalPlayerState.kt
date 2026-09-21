package com.lelloman.pezzottify.android.localplayer

data class LocalPlayerState(
    val isPlaying: Boolean = false,
    val currentTrackIndex: Int = 0,
    val queue: List<LocalTrackInfo> = emptyList(),
    val progressPercent: Float = 0f,
    val progressSeconds: Int = 0,
    val durationSeconds: Int = 0
) {
    val currentTrack: LocalTrackInfo?
        get() = queue.getOrNull(currentTrackIndex)

    val hasNext: Boolean
        get() = currentTrackIndex < queue.size - 1

    val hasPrevious: Boolean
        get() = currentTrackIndex > 0

    val isEmpty: Boolean
        get() = queue.isEmpty()
}

data class LocalTrackInfo(
    val uri: String,
    val displayName: String
)
