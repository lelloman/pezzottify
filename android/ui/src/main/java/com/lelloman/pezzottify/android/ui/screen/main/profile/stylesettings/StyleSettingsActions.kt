package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.ThemeMode

interface StyleSettingsActions {
    fun selectThemeMode(themeMode: ThemeMode)
    fun selectColorPalette(colorPalette: ColorPalette)
    fun selectFontFamily(fontFamily: AppFontFamily)
}
