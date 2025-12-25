package com.lelloman.pezzottify.android.ui.screen.main.search

import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType

interface SearchScreenActions {

    fun updateQuery(query: String)

    fun toggleFilter(filter: SearchFilter)

    fun clickOnArtistSearchResult(artistId: String)

    fun clickOnAlbumSearchResult(albumId: String)

    fun clickOnTrackSearchResult(trackId: String)

    fun clickOnRecentlyViewedItem(itemId: String, itemType: ViewedContentType)

    fun clickOnSearchHistoryItem(itemId: String, itemType: ViewedContentType)

    fun clickOnWhatsNewAlbum(albumId: String)

    fun clickOnWhatsNewSeeAll()
}