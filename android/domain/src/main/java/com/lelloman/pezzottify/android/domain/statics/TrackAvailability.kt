package com.lelloman.pezzottify.android.domain.statics

/**
 * Track availability state for on-demand downloading.
 * Matches the server's TrackAvailability enum.
 */
enum class TrackAvailability {
    /** Track is available for streaming */
    Available,

    /** Track is not available (file missing) */
    Unavailable,

    /** Track is currently being fetched */
    Fetching,

    /** Track fetch failed */
    FetchError;

    companion object {
        /**
         * Parse from server's snake_case string representation.
         * Defaults to Available for unknown values.
         */
        fun fromServerString(value: String?): TrackAvailability {
            return when (value) {
                "available" -> Available
                "unavailable" -> Unavailable
                "fetching" -> Fetching
                "fetch_error" -> FetchError
                else -> Available
            }
        }
    }

    /**
     * Whether the track can be played.
     */
    val isPlayable: Boolean
        get() = this == Available
}
