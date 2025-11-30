package com.lelloman.pezzottify.android.domain.statics

interface Album : StaticItem {
    val id: String
    val name: String
    val date: Long
    val genre: List<String>
    val displayImageId: String?
    val related: List<String>
    val artistsIds: List<String>
    val discs: List<Disc>
}

interface Disc {
    val name: String?
    val tracksIds: List<String>
}