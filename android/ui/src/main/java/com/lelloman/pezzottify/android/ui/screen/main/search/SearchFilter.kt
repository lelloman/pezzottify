package com.lelloman.pezzottify.android.ui.screen.main.search

/**
 * Filter types for search results.
 * Can be used for both catalog search and external search.
 */
enum class SearchFilter {
    Album,
    Artist,
    Track;

    val displayName: String
        get() = when (this) {
            Album -> "Albums"
            Artist -> "Artists"
            Track -> "Tracks"
        }

    companion object {
        /** Filters available for catalog search (all types) */
        val catalogFilters = listOf(Album, Artist, Track)

        /** Filters available for external search (no Track support) */
        val externalFilters = listOf(Album, Artist)
    }
}
