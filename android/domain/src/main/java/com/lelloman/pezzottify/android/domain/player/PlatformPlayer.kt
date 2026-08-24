package com.lelloman.pezzottify.android.domain.player

interface PlatformPlayer : ControlsAndStatePlayer {

    fun loadPlaylist(tracksUrls: List<String>, playWhenReady: Boolean = true)

    fun addMediaItems(tracksUrls: List<String>)

    fun removeMediaItem(index: Int)

    /**
     * Loads a track at [index] and seeks to [positionMs].
     *
     * Unlike [loadTrackIndex], this is also used while a controller is still being created,
     * so implementations must retain both values until the playlist is ready.
     */
    fun loadTrack(index: Int, positionMs: Long)

    /**
     * Clears the player session completely, stopping playback and removing all media items.
     * Used during logout to fully reset the player state.
     */
    fun clearSession()

}
