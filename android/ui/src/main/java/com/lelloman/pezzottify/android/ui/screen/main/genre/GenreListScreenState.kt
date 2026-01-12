package com.lelloman.pezzottify.android.ui.screen.main.genre

data class GenreListScreenState(
    val genres: List<GenreListItemState> = emptyList(),
    val filteredGenres: List<GenreListItemState> = emptyList(),
    val searchQuery: String = "",
    val isLoading: Boolean = false,
    val error: String? = null,
)

data class GenreListItemState(
    val name: String,
    val trackCount: Int,
)
