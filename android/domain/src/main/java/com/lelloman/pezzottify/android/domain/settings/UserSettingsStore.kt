package com.lelloman.pezzottify.android.domain.settings

import kotlinx.coroutines.flow.StateFlow

interface UserSettingsStore {

    val playBehavior: StateFlow<PlayBehavior>
    val themeMode: StateFlow<ThemeMode>

    suspend fun setPlayBehavior(playBehavior: PlayBehavior)
    suspend fun setThemeMode(themeMode: ThemeMode)
}

enum class PlayBehavior {
    ReplacePlaylist,
    AddToPlaylist;

    companion object {
        val Default = ReplacePlaylist
    }
}

enum class ThemeMode {
    System,
    Light,
    Dark;

    companion object {
        val Default = System
    }
}
