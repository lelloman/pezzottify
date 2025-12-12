package com.lelloman.pezzottify.android.ui.screen.main.search

sealed class ExternalSearchResultContent {
    abstract val id: String
    abstract val name: String
    abstract val imageUrl: String?
    abstract val inCatalog: Boolean
    abstract val inQueue: Boolean
    abstract val catalogId: String?
    abstract val score: Float

    data class Album(
        override val id: String,
        override val name: String,
        val artistName: String,
        val year: Int?,
        override val imageUrl: String?,
        override val inCatalog: Boolean,
        override val inQueue: Boolean,
        override val catalogId: String?,
        override val score: Float,
    ) : ExternalSearchResultContent()

    data class Artist(
        override val id: String,
        override val name: String,
        override val imageUrl: String?,
        override val inCatalog: Boolean,
        override val inQueue: Boolean,
        override val catalogId: String?,
        override val score: Float,
    ) : ExternalSearchResultContent()

    data class Track(
        override val id: String,
        override val name: String,
        val artistName: String,
        val albumName: String?,
        val duration: Int?,
        override val imageUrl: String?,
        override val inCatalog: Boolean,
        override val inQueue: Boolean,
        override val catalogId: String?,
        override val score: Float,
    ) : ExternalSearchResultContent()
}
