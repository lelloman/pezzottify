package com.lelloman.pezzottify.android.domain.playback

import kotlinx.coroutines.flow.StateFlow

/**
 * Manager for remote playback across devices (Spotify-style).
 *
 * Key concept: This device can select where audio plays:
 * - Local (this device) - we control local player directly
 * - Remote (another device) - we send commands to that device
 *
 * The UI always acts as the controller. The selectedOutputDevice determines
 * where commands are routed.
 */
interface RemotePlaybackManager {

    /**
     * This device's ID assigned by the server.
     */
    val deviceId: StateFlow<Long?>

    /**
     * List of all connected devices (including this one).
     */
    val devices: StateFlow<List<PlaybackDevice>>

    /**
     * Currently selected output device ID.
     * null = local output (this device plays audio)
     * deviceId = remote output (that device plays audio)
     */
    val selectedOutputDevice: StateFlow<Long?>

    /**
     * Whether a playback session exists.
     */
    val sessionExists: StateFlow<Boolean>

    /**
     * Remote playback state (when outputting to remote device).
     */
    val remoteState: StateFlow<PlaybackState?>

    /**
     * Remote queue (when outputting to remote device).
     */
    val remoteQueue: StateFlow<List<QueueItem>>

    /**
     * Interpolated position for smooth display when remote.
     */
    val interpolatedPosition: StateFlow<Float>

    /**
     * Whether this is a local output (audio plays on this device).
     */
    val isLocalOutput: StateFlow<Boolean>

    /**
     * Whether this device is the audio device in the session.
     */
    val isAudioDevice: StateFlow<Boolean>

    // ============================================
    // Output device selection
    // ============================================

    /**
     * Select output device.
     * @param deviceId null for local, or remote device ID
     */
    fun selectOutputDevice(deviceId: Long?)

    // ============================================
    // Unified playback commands (route based on output)
    // ============================================

    fun play()
    fun pause()
    fun playPause()
    fun seek(positionSec: Float)
    fun seekToPercentage(percent: Float)
    fun skipNext()
    fun skipPrevious()
    fun forward10Sec()
    fun rewind10Sec()
    fun setVolume(volume: Float)
    fun setMuted(muted: Boolean)
    fun stop()

    // ============================================
    // Internal (called by player when local output)
    // ============================================

    /**
     * Register this device as the audio device (called when playback starts).
     */
    fun registerAsAudioDevice()

    /**
     * Unregister this device as the audio device.
     */
    fun unregisterAsAudioDevice()

    /**
     * Broadcast current playback state (called periodically by player).
     */
    fun broadcastState(state: PlaybackState)

    /**
     * Broadcast queue update.
     */
    fun broadcastQueue(queue: List<QueueItem>, version: Int)
}
