package com.lelloman.pezzottify.android.ui.screen.main.search

sealed interface SearchScreensEvents {

    data class NavigateToArtistScreen(val artistId: String) : SearchScreensEvents

    data class NavigateToAlbumScreen(val albumId: String) : SearchScreensEvents

    data class NavigateToTrackScreen(val trackId: String) : SearchScreensEvents

}