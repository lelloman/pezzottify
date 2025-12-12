package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.annotation.StringRes
import com.lelloman.pezzottify.android.ui.R

/**
 * Filter types for search results.
 * Can be used for both catalog search and external search.
 */
enum class SearchFilter(@StringRes val displayNameRes: Int) {
    Album(R.string.filter_albums),
    Artist(R.string.filter_artists),
    Track(R.string.filter_tracks);

    companion object {
        /** Filters available for catalog search (all types) */
        val catalogFilters = listOf(Album, Artist, Track)

        /** Filters available for external search */
        val externalFilters = listOf(Album, Artist, Track)
    }
}
