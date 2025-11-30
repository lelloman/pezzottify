package com.lelloman.pezzottify.android.ui.screen.main.content.album

interface AlbumScreenActions {

    fun clickOnPlayAlbum(albumId: String)
    fun clickOnTrack(trackId: String)
    fun clickOnAlbumImage(imageUrl: String?)
    fun clickOnLike()
}