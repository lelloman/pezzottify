package com.lelloman.pezzottify.android.domain.player

interface PlatformPlayer : ControlsAndStatePlayer {

    fun loadPlaylist(tracksUrls: List<String>)

}