package com.lelloman.pezzottify.android.ui.screen.main.whatsnew

sealed interface WhatsNewScreenEvents {
    data class NavigateToAlbum(val albumId: String) : WhatsNewScreenEvents
    data object NavigateBack : WhatsNewScreenEvents
}
