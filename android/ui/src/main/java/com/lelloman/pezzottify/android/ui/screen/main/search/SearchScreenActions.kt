package com.lelloman.pezzottify.android.ui.screen.main.search

interface SearchScreenActions {

    fun updateQuery(query: String)

    fun clickOnArtistSearchResult(artistId: String)
}