package com.lelloman.pezzottify.android.ui.content

data class ArtistInfo(
    val id: String,
    val name: String,
)

/**
 * Track availability state for on-demand downloading (UI layer).
 */
enum class TrackAvailability {
    Available,
    Unavailable,
    Fetching,
    FetchError;

    val isPlayable: Boolean
        get() = this == Available
}

data class Track(
    val id: String,
    val name: String,
    val albumId: String,
    val artists: List<ArtistInfo>,
    val durationSeconds: Int,
    val availability: TrackAvailability = TrackAvailability.Available,
) {
    /** Whether the track can be played */
    val isPlayable: Boolean
        get() = availability.isPlayable

    /** Whether the track is currently being fetched */
    val isFetching: Boolean
        get() = availability == TrackAvailability.Fetching

    /** Whether the track fetch failed */
    val isFetchError: Boolean
        get() = availability == TrackAvailability.FetchError

    /** Whether the track is unavailable (any reason) */
    val isUnavailable: Boolean
        get() = !isPlayable
}