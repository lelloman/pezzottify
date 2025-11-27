package com.lelloman.pezzottify.android.ui.screen.player

import com.lelloman.pezzottify.android.ui.content.ArtistInfo

enum class RepeatModeUi {
    OFF,
    ALL,
    ONE
}

data class PlayerScreenState(
    val isLoading: Boolean = true,
    val trackName: String = "",
    val albumId: String = "",
    val albumName: String = "",
    val albumImageUrls: List<String> = emptyList(),
    val artists: List<ArtistInfo> = emptyList(),
    val isPlaying: Boolean = false,
    val trackProgressPercent: Float = 0f,
    val trackProgressSec: Int = 0,
    val trackDurationSec: Int = 0,
    val hasNextTrack: Boolean = false,
    val hasPreviousTrack: Boolean = false,
    val volume: Float = 0.5f,
    val isMuted: Boolean = false,
    val shuffleEnabled: Boolean = false,
    val repeatMode: RepeatModeUi = RepeatModeUi.OFF,
)
