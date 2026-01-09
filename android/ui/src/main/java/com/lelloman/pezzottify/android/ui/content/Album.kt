package com.lelloman.pezzottify.android.ui.content

/**
 * Album availability state based on track audio file presence (UI layer).
 */
enum class AlbumAvailability {
    /** No tracks have audio files available */
    Missing,
    /** Some tracks have audio, some are missing */
    Partial,
    /** All tracks have audio files available */
    Complete;

    /** Whether the album has any playable tracks */
    val hasPlayableTracks: Boolean
        get() = this != Missing

    /** Whether all tracks in the album are available */
    val isFullyAvailable: Boolean
        get() = this == Complete
}

data class Album(
    val id: String,
    val name: String,
    val date: Int,
    val imageUrl: String?,
    val artistsIds: List<String>,
    val discs: List<Disc> = emptyList(),
    val availability: AlbumAvailability = AlbumAvailability.Missing,
)

data class Disc(
    val tracksIds: List<String>,
)