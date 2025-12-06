package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

data class StyleSettingsState(
    val themeMode: ThemeMode = ThemeMode.Default,
    val colorPalette: ColorPalette = ColorPalette.Default,
    val fontFamily: AppFontFamily = AppFontFamily.Default,
)
