package com.lelloman.pezzottify.android.domain.player

import android.os.Looper
import androidx.annotation.FloatRange

interface PlatformPlayer {

    fun setIsPlaying(isPlaying: Boolean)

    fun loadPlaylist(tracksUrls: List<String>)

    fun loadTrackIndex(loadTrackIndex: Int)

    fun seekTrackProgressPercent(@FloatRange(from = 0.0, to = 1.0) trackProgressPercent: Float)

    interface Factory {
        fun create(looper: Looper): PlatformPlayer
    }
}