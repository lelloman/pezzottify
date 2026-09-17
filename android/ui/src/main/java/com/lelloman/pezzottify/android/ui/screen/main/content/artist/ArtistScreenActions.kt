package com.lelloman.pezzottify.android.ui.screen.main.content.artist

interface ArtistScreenActions {

    fun clickOnLike()

    fun clickOnArtistImage(imageUrl: String?)

    fun clickOnRadio()

    fun clickOnGreatestHits()

    fun loadMoreAlbums()

    fun loadMoreFeatures()
}
