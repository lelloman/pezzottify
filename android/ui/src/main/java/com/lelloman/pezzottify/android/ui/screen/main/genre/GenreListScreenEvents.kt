package com.lelloman.pezzottify.android.ui.screen.main.genre

sealed interface GenreListScreenEvents {
    data class NavigateToGenre(val genreName: String) : GenreListScreenEvents
    data object NavigateBack : GenreListScreenEvents
}
