package com.lelloman.pezzottify.android.ui.content

data class Album(
    val id: String,
    val name: String,
    val date: Int,
    val imageUrl: String?,
    val artistsIds: List<String>,
    val discs: List<Disc> = emptyList(),
)

data class Disc(
    val tracksIds: List<String>,
)