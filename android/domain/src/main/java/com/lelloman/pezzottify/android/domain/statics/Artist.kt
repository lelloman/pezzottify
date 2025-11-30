package com.lelloman.pezzottify.android.domain.statics

interface Artist : StaticItem {
    val id: String
    val name: String
    val displayImageId: String?
    val related: List<String>
}