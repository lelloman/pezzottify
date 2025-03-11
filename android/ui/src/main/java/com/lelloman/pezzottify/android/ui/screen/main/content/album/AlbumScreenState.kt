package com.lelloman.pezzottify.android.ui.screen.main.content.album

import com.lelloman.pezzottify.android.ui.content.Album

data class AlbumScreenState(
    val album: Album? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
)