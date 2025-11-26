package com.lelloman.pezzottify.android.ui.content

data class Artist(
    val id: String,
    val name: String,
    val imageUrls: List<String>,
    val related: List<String>,
)