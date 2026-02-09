package com.lelloman.pezzottify.android.ui.screen.queue

import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import kotlinx.coroutines.flow.Flow

interface QueueScreenActions {

    fun clickOnTrack(index: Int)
    fun moveTrack(fromIndex: Int, toIndex: Int)
    fun removeTrack(index: Int)
    fun clickOnSaveAsPlaylist()

    // Track actions bottom sheet
    fun playTrackDirectly(trackId: String)
    fun addTrackToQueue(trackId: String)
    fun addTrackToPlaylist(trackId: String, targetPlaylistId: String)
    fun createPlaylist(name: String)
    fun toggleTrackLike(trackId: String, currentlyLiked: Boolean)
    fun getTrackLikeState(trackId: String): Flow<Boolean>
    fun getUserPlaylists(): Flow<List<UiUserPlaylist>>
}
