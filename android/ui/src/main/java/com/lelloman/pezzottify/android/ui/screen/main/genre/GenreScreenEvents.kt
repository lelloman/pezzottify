package com.lelloman.pezzottify.android.ui.screen.main.genre

sealed interface GenreScreenEvents {
    data class NavigateToTrack(val trackId: String) : GenreScreenEvents
    data object NavigateBack : GenreScreenEvents
}
