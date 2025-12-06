package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

interface StyleSettingsActions {
    fun selectThemeMode(themeMode: ThemeMode)
    fun selectColorPalette(colorPalette: ColorPalette)
    fun selectFontFamily(fontFamily: AppFontFamily)
}
