package com.lelloman.pezzottify.android.domain.player

import kotlinx.coroutines.flow.StateFlow

interface ControlsAndStatePlayer {

    val isActive: StateFlow<Boolean>
    val isPlaying: StateFlow<Boolean>
    val volumeState: StateFlow<VolumeState>
    val currentTrackIndex: StateFlow<Int?>
    val currentTrackPercent: StateFlow<Float?>
    val currentTrackProgressSec: StateFlow<Int?>

    fun togglePlayPause()
    fun seekToPercentage(percentage: Float)
    fun setIsPlaying(isPlaying: Boolean)
    fun forward10Sec()
    fun rewind10Sec()
    fun stop()
    fun setVolume(volume: Float)
    fun setMuted(isMuted: Boolean)
    fun loadTrackIndex(index: Int)
    fun skipToNextTrack()
    fun skipToPreviousTrack()
}