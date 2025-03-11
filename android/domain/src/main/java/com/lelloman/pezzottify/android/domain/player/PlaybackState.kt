package com.lelloman.pezzottify.android.domain.player

import androidx.annotation.FloatRange


data class PlaybackState(
    val currentTrackIndex: Int?,
    val isPlaying: Boolean,
    @FloatRange(0.0, 1.0) val currentTrackPercent: Float,
    val progressSec: Int?,
    @FloatRange(0.0, 1.0) val volume: Float,
    val isMuted: Boolean,
)