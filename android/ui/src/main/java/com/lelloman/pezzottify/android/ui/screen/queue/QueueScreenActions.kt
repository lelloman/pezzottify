package com.lelloman.pezzottify.android.ui.screen.queue

interface QueueScreenActions {

    fun clickOnTrack(index: Int)
    fun moveTrack(fromIndex: Int, toIndex: Int)
    fun removeTrack(trackId: String)
    fun clickOnSaveAsPlaylist()
}
