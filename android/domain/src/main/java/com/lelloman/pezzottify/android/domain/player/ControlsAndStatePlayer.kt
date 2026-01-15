package com.lelloman.pezzottify.android.domain.player

import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow

interface ControlsAndStatePlayer {

    val isActive: StateFlow<Boolean>
    val isPlaying: StateFlow<Boolean>
    val volumeState: StateFlow<VolumeState>
    val currentTrackIndex: StateFlow<Int?>
    val currentTrackPercent: StateFlow<Float?>
    val currentTrackProgressSec: StateFlow<Int?>

    /**
     * Duration of the currently playing track in seconds.
     * Populated when track info is loaded/fetched.
     * Used by ListeningTracker for listening stats.
     */
    val currentTrackDurationSeconds: StateFlow<Int?>

    /**
     * Emits when a seek operation occurs (seekToPercentage, forward10Sec, rewind10Sec).
     * Used by ListeningTracker to count seeks.
     */
    val seekEvents: SharedFlow<SeekEvent>

    /**
     * Current player error state, if any.
     * Set when playback encounters an error, cleared on recovery or retry.
     */
    val playerError: StateFlow<PlayerError?>

    data class SeekEvent(val timestamp: Long)
    data class PlayerError(
        val trackId: String?,
        val message: String,
        val errorCode: String?,
        val isRecoverable: Boolean, // True for transient errors (network), false for permanent errors (codec)
        val positionMs: Long? = null // Last known playback position before error
    )
    val shuffleEnabled: StateFlow<Boolean>
    val repeatMode: StateFlow<RepeatMode>

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
    fun toggleShuffle()
    fun cycleRepeatMode()

    /**
     * Retry playback after an error.
     * Resumes from the last position if available.
     */
    fun retry()
}