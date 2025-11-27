package com.lelloman.pezzottify.android.ui.content

data class ArtistInfo(
    val id: String,
    val name: String,
)

data class Track(
    val id: String,
    val name: String,
    val albumId: String,
    val artists: List<ArtistInfo>,
    val durationSeconds: Int,
)