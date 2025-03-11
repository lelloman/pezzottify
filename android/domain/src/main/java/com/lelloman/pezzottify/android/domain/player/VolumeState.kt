package com.lelloman.pezzottify.android.domain.player

import androidx.annotation.FloatRange

data class VolumeState(
    @FloatRange(0.0, 1.0) val volume: Float,
    val isMuted: Boolean,
)