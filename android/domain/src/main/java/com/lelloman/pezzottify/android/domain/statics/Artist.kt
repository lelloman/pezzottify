package com.lelloman.pezzottify.android.domain.statics

interface Artist : StaticItem {
    val id: String
    val name: String
    val portraits: List<Image>
    val portraitGroup: List<Image>
}