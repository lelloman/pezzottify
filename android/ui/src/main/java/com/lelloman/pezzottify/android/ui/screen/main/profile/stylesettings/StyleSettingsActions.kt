package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette

interface StyleSettingsActions {
    fun selectColorPalette(colorPalette: ColorPalette)
    fun selectFontFamily(fontFamily: AppFontFamily)
}
