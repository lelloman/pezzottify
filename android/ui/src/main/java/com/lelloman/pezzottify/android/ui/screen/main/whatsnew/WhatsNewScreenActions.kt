package com.lelloman.pezzottify.android.ui.screen.main.whatsnew

interface WhatsNewScreenActions {
    fun clickOnAlbum(albumId: String)
    fun toggleBatchExpanded(batchId: String)
    fun goBack()
}
