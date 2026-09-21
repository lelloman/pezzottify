package com.lelloman.pezzottify.android.ui.theme

enum class ThemeMode {
    System,
    Light,
    Dark,
    Amoled;

    companion object {
        val Default = Dark
    }
}

enum class ColorPalette {
    Classic,
    OceanBlue,
    SunsetCoral,
    PurpleHaze,
    RoseGold,
    Midnight,
    Forest;

    companion object {
        val Default = Classic
    }
}

enum class AppFontFamily {
    System,
    SansSerif,
    Serif,
    Monospace;

    companion object {
        val Default = SansSerif
    }
}
