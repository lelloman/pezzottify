package com.lelloman.pezzottify.android.ui.content


sealed class SearchResultContent {
    abstract val id: String

    class Artist(override val id: String, val name: String, val imageUrl: String?) : SearchResultContent()

    class Album(
        override val id: String,
        val name: String,
        val artistsIds: List<String>,
        val imageUrl: String?
    ) : SearchResultContent()

    class Track(
        override val id: String,
        val name: String,
        val artistsIds: List<String>,
        val durationSeconds: Int,
        val albumId: String,
    ) : SearchResultContent()
}