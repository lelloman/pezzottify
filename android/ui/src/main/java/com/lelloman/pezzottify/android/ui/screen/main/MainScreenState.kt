package com.lelloman.pezzottify.android.ui.screen.main

import com.lelloman.pezzottify.android.ui.content.ArtistInfo

data class MainScreenState(
    val bottomPlayer: BottomPlayer = BottomPlayer(),
    val notificationUnreadCount: Int = 0,
) {

    data class BottomPlayer(
        val isVisible: Boolean = false,
        val isLoading: Boolean = false,
        val trackId: String = "",
        val trackName: String = "",
        val albumName: String = "",
        val albumImageUrl: String? = null,
        val artists: List<ArtistInfo> = emptyList(),
        val isPlaying: Boolean = false,
        val trackPercent: Float = 0f,
        val nextTrackName: String? = null,
        val nextTrackArtists: List<ArtistInfo> = emptyList(),
        val previousTrackName: String? = null,
        val previousTrackArtists: List<ArtistInfo> = emptyList(),
    )
}