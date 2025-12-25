package com.lelloman.pezzottify.android.domain.player

/**
 * Persists playback state so it can be restored if the service is killed by the system.
 *
 * This is used to recover from situations where Android kills the player service
 * while the app is in the background with playback paused.
 */
interface PlaybackStateStore {

    /**
     * Save the current playback state.
     *
     * @param playlist The current playlist (context + track IDs)
     * @param currentTrackIndex The index of the currently playing track
     * @param positionMs The playback position in milliseconds
     * @param isPlaying Whether playback was active
     */
    suspend fun saveState(
        playlist: PlaybackPlaylist,
        currentTrackIndex: Int,
        positionMs: Long,
        isPlaying: Boolean,
    )

    /**
     * Load the saved playback state.
     *
     * @return The saved state, or null if none exists or it has expired
     */
    suspend fun loadState(): SavedPlaybackState?

    /**
     * Clear the saved state (e.g., after successful restoration or on logout).
     */
    suspend fun clearState()
}

/**
 * Represents a saved playback state that can be restored.
 */
data class SavedPlaybackState(
    val playlist: PlaybackPlaylist,
    val currentTrackIndex: Int,
    val positionMs: Long,
    val isPlaying: Boolean,
    val savedAtMs: Long,
)
