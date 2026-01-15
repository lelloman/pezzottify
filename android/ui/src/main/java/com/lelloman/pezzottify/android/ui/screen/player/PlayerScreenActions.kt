package com.lelloman.pezzottify.android.ui.screen.player

interface PlayerScreenActions {
    fun clickOnPlayPause()
    fun clickOnSkipNext()
    fun clickOnSkipPrevious()
    fun seekToPercent(percent: Float)
    fun setVolume(volume: Float)
    fun toggleMute()
    fun clickOnShuffle()
    fun clickOnRepeat()
    fun retry()
}
