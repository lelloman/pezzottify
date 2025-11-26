package com.lelloman.pezzottify.android.ui.content

data class Album(
    val id: String,
    val name: String,
    val date: Long,
    val imageUrls: List<String>,
    val artistsIds: List<String>,
    val discs: List<Disc> = emptyList(),
)

data class Disc(
    val name: String?,
    val tracksIds: List<String>,
)