package com.lelloman.pezzottify.android.domain.statics

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Track availability state for on-demand downloading.
 * Matches the server's TrackAvailability enum.
 */
@Serializable
enum class TrackAvailability {
    /** Track is available for streaming */
    @SerialName("available")
    Available,

    /** Track is not available (file missing) */
    @SerialName("unavailable")
    Unavailable,

    /** Track is currently being fetched */
    @SerialName("fetching")
    Fetching,

    /** Track fetch failed */
    @SerialName("fetch_error")
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
