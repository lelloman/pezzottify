package com.lelloman.pezzottify.android.domain.statics

interface Album : StaticItemType {
    val id: String
    val name: String
    val genre: List<String>
    val portraitsImagesIds: List<String>
    val related: List<String>
    val portraitGroupImagesIds: List<String>
}