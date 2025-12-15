package com.lelloman.pezzottify.android.ui.screen.main.content.album

import kotlinx.coroutines.flow.Flow

interface AlbumScreenActions {

    fun clickOnPlayAlbum(albumId: String)
    fun clickOnTrack(trackId: String)
    fun clickOnAlbumImage(imageUrl: String?)
    fun clickOnLike()

    // New actions for bottom sheets
    fun playTrackDirectly(trackId: String)
    fun addTrackToQueue(trackId: String)
    fun addAlbumToQueue(albumId: String)
    fun addTrackToPlaylist(trackId: String, playlistId: String)
    fun addAlbumToPlaylist(albumId: String, playlistId: String)
    fun createPlaylist(name: String)
    fun toggleTrackLike(trackId: String, currentlyLiked: Boolean)
    fun getTrackLikeState(trackId: String): Flow<Boolean>
}
