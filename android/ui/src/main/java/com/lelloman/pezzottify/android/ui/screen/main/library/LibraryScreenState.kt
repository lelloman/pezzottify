package com.lelloman.pezzottify.android.ui.screen.main.library

data class LibraryScreenState(
    val likedAlbumIds: List<String> = emptyList(),
    val likedArtistIds: List<String> = emptyList(),
    val isLoading: Boolean = true,
)
