package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import com.lelloman.pezzottify.android.ui.content.Artist

data class ArtistScreenState(
    val artist: Artist? = null,
    val albums: List<String> = emptyList(),
    val features: List<String> = emptyList(),
    val relatedArtists: List<String> = emptyList(),
    val externalAlbums: List<UiExternalAlbumItem> = emptyList(),
    val isExternalAlbumsError: Boolean = false,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val isLiked: Boolean = false,
)

/**
 * External album item for display in artist discography.
 * Shows albums not yet in the catalog.
 */
data class UiExternalAlbumItem(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val year: Int?,
    /** Whether this album is currently in the download queue */
    val inQueue: Boolean,
)