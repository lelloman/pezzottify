package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import com.lelloman.pezzottify.android.ui.content.Artist

data class ArtistScreenState(
    val artist: Artist? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
)