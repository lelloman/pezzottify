package com.lelloman.pezzottify.android.ui.screen.main.home

sealed interface HomeScreenEvents {
    data object NavigateToProfileScreen : HomeScreenEvents

    data class NavigateToArtist(val artistId: String) : HomeScreenEvents
    data class NavigateToAlbum(val albumId: String) : HomeScreenEvents
    data class NavigateToTrack(val trackId: String) : HomeScreenEvents
}