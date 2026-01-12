package com.lelloman.pezzottify.android.ui.screen.main.genre

data class GenreScreenState(
    val genreName: String = "",
    val tracks: List<GenreTrackItemState> = emptyList(),
    val totalTracks: Int = 0,
    val isLoading: Boolean = false,
    val error: String? = null,
)

data class GenreTrackItemState(
    val id: String,
    val name: String,
    val durationMs: Long,
    val albumId: String,
    val albumName: String,
    val artistNames: List<String>,
    val albumImageUrl: String?,
)
