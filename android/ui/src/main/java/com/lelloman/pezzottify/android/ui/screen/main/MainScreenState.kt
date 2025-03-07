package com.lelloman.pezzottify.android.ui.screen.main

data class MainScreenState(
    val tab: Tab = Tab.Home,
) {

    enum class Tab {
        Home,
        Search,
        Library,
    }
}