package com.lelloman.pezzottify.android.ui.screen.main.library

data class LibraryScreenState(
    val likedAlbumIds: List<String> = emptyList(),
    val likedArtistIds: List<String> = emptyList(),
    val likedTrackIds: List<String> = emptyList(),
    val playlists: List<UiUserPlaylist> = emptyList(),
    val isLoading: Boolean = true,
)

data class UiUserPlaylist(
    val id: String,
    val name: String,
    val trackCount: Int,
)
