package com.lelloman.pezzottify.android.domain.statics

interface Track : StaticItemType {
    val id: String
    val name: String
    val albumId: String
    val artistsIds: List<String>
    val durationSeconds: Int
}