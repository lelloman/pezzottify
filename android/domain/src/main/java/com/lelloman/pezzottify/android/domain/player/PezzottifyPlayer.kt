package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import kotlinx.coroutines.flow.StateFlow

interface PezzottifyPlayer : ControlsAndStatePlayer, AppInitializer{

    val playbackPlaylist: StateFlow<PlaybackPlaylist?>
    val canGoToPreviousPlaylist: StateFlow<Boolean>
    val canGoToNextPlaylist: StateFlow<Boolean>

    fun loadAlbum(albumId: String, startTrackId: String? = null)
    fun addAlbumToPlaylist(albumId: String)
    fun loadUserPlaylist(userPlaylistId: String, startTrackId: String? = null)
    fun addUserPlaylistToQueue(userPlaylistId: String)
    fun loadSingleTrack(trackId: String)


    fun goToPreviousPlaylist()
    fun goToNextPlaylist()
    fun moveTrack(fromIndex: Int, toIndex: Int)
    fun addTracksToPlaylist(tracksIds: List<String>)
    fun removeTrackFromPlaylist(trackId: String)

    /**
     * Clears the player session completely, stopping playback and clearing the playlist.
     * Used during logout to fully reset the player state.
     */
    fun clearSession()

    /**
     * Attempts to restore a previously saved playback state.
     * Called when the player service was killed by the system while paused.
     *
     * @return true if restoration was attempted, false if no saved state exists
     */
    suspend fun tryRestoreState(): Boolean

}

