package com.lelloman.pezzottify.android.ui.screen.main.home

interface HomeScreenActions {

    suspend fun clickOnProfile()

    fun clickOnRecentlyViewedItem(itemId: String, itemType: ViewedContentType)
}