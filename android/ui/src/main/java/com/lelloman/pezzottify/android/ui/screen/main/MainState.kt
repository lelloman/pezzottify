package com.lelloman.pezzottify.android.ui.screen.main

data class MainState(
    val tab: Tab = Tab.Home,
) {

    enum class Tab {
        Home,
        Search,
        Library,
    }
}