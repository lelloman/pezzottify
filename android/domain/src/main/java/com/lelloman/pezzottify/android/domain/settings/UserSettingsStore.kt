package com.lelloman.pezzottify.android.domain.settings

import kotlinx.coroutines.flow.StateFlow

interface UserSettingsStore {

    val playBehavior: StateFlow<PlayBehavior>
    val themeMode: StateFlow<ThemeMode>
    val colorPalette: StateFlow<ColorPalette>
    val fontFamily: StateFlow<AppFontFamily>

    suspend fun setPlayBehavior(playBehavior: PlayBehavior)
    suspend fun setThemeMode(themeMode: ThemeMode)
    suspend fun setColorPalette(colorPalette: ColorPalette)
    suspend fun setFontFamily(fontFamily: AppFontFamily)
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
    Dark,
    Amoled;  // True black for OLED screens

    companion object {
        val Default = System
    }
}

enum class ColorPalette {
    Classic,      // Original green theme
    OceanBlue,    // Cool blue tones
    SunsetCoral,  // Warm coral/orange tones
    PurpleHaze,   // Violet/purple theme
    RoseGold,     // Warm pink/rose tones
    Midnight,     // Deep navy/indigo tones
    Forest;       // Earthy green/brown tones

    companion object {
        val Default = Classic
    }
}

enum class AppFontFamily {
    System,       // Default system fonts
    SansSerif,    // Clean sans-serif
    Serif,        // Classic serif style
    Monospace;    // Monospace for a developer feel

    companion object {
        val Default = System
    }
}
