package com.lelloman.pezzottify.android.ui.screen.main

data class MainScreenState(
    val bottomPlayer: BottomPlayer = BottomPlayer(),
) {

    data class BottomPlayer(
        val isVisible: Boolean = false,
        val trackId: String = "",
        val trackName: String = "",
        val artistsNames: String = "",
        val isPlaying: Boolean = false,
    )
}