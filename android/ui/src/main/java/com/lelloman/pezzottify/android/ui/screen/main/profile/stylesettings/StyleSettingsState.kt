package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.ThemeMode

data class StyleSettingsState(
    val themeMode: ThemeMode = ThemeMode.Default,
    val colorPalette: ColorPalette = ColorPalette.Default,
    val fontFamily: AppFontFamily = AppFontFamily.Default,
)
