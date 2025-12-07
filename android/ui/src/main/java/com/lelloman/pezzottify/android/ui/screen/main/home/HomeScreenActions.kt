package com.lelloman.pezzottify.android.ui.screen.main.home

interface HomeScreenActions {

    suspend fun clickOnProfile()

    suspend fun clickOnSettings()

    fun clickOnRecentlyViewedItem(itemId: String, itemType: ViewedContentType)

    fun clickOnPopularAlbum(albumId: String)

    fun clickOnPopularArtist(artistId: String)
}