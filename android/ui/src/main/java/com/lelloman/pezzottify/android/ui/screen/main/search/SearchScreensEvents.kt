package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.annotation.StringRes

sealed interface SearchScreensEvents {

    data class NavigateToArtistScreen(val artistId: String) : SearchScreensEvents

    data class NavigateToAlbumScreen(val albumId: String) : SearchScreensEvents

    data class NavigateToTrackScreen(val trackId: String) : SearchScreensEvents

    data class ShowRequestError(@StringRes val messageRes: Int) : SearchScreensEvents

    data object ShowRequestSuccess : SearchScreensEvents

    data class NavigateToExternalAlbumScreen(val albumId: String) : SearchScreensEvents

    data class NavigateToExternalArtistScreen(val artistId: String) : SearchScreensEvents

    data class NavigateToExternalTrackScreen(val trackId: String) : SearchScreensEvents

    data class ShowMessage(@StringRes val messageRes: Int) : SearchScreensEvents

}