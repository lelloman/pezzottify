package com.lelloman.pezzottify.android.domain.statics

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Album availability state based on track audio file presence.
 * Matches the server's AlbumAvailability enum.
 */
@Serializable
enum class AlbumAvailability {
    /** No tracks have audio files available */
    @SerialName("missing")
    Missing,

    /** Some tracks have audio, some are missing */
    @SerialName("partial")
    Partial,

    /** All tracks have audio files available */
    @SerialName("complete")
    Complete;

    companion object {
        /**
         * Parse from server's snake_case string representation.
         * Defaults to Missing for unknown values.
         */
        fun fromServerString(value: String?): AlbumAvailability {
            return when (value) {
                "complete" -> Complete
                "partial" -> Partial
                "missing" -> Missing
                else -> Missing
            }
        }
    }

    /**
     * Whether the album has any playable tracks.
     */
    val hasPlayableTracks: Boolean
        get() = this != Missing

    /**
     * Whether all tracks in the album are available.
     */
    val isFullyAvailable: Boolean
        get() = this == Complete
}
