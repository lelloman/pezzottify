package com.lelloman.pezzottify.android.ui.screen.main

interface MainScreenActions {
    fun cancelPendingPlayback() {}
    fun retryPendingPlayback() {}
    fun retryRadioContinuation() {}
    fun dismissRadioContinuationError() {}

    fun clickOnPlayPause()
    
    fun clickOnSkipToNext()

    fun clickOnSkipToPrevious()
}
