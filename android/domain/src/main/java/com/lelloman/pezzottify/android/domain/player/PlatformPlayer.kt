package com.lelloman.pezzottify.android.domain.player

interface PlatformPlayer : ControlsAndStatePlayer {

    fun loadPlaylist(tracks: List<MediaTrackInfo>)

    fun addMediaItems(tracks: List<MediaTrackInfo>)

    fun removeMediaItem(index: Int)

    /**
     * Clears the player session completely, stopping playback and removing all media items.
     * Used during logout to fully reset the player state.
     */
    fun clearSession()

}