package com.lelloman.pezzottify.android.domain.statics

data class FeaturedAlbums(
    val heroIndex: Int,
    val albums: List<FeaturedAlbum>,
)

data class FeaturedAlbum(
    val id: String,
    val name: String,
    val artistNames: List<String>,
)
