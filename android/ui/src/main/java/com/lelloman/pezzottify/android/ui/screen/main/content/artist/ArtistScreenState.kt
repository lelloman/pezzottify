package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import com.lelloman.pezzottify.android.ui.content.Artist

data class ArtistScreenState(
    val artist: Artist? = null,
    val albums: List<String> = emptyList(),
    val features: List<String> = emptyList(),
    val relatedArtists: List<String> = emptyList(),
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val isLiked: Boolean = false,
)