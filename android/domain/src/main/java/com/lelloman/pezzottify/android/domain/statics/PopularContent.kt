package com.lelloman.pezzottify.android.domain.statics

/**
 * Popular albums and artists based on listening data.
 */
data class PopularContent(
    val albums: List<PopularAlbum>,
    val artists: List<PopularArtist>,
)

data class PopularAlbum(
    val id: String,
    val name: String,
    val artistNames: List<String>,
)

data class PopularArtist(
    val id: String,
    val name: String,
)
