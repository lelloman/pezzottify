package com.lelloman.pezzottify.android.ui.screen.main.settings

import com.lelloman.pezzottify.android.ui.model.PlayBehavior
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

interface SettingsScreenActions {

    fun selectPlayBehavior(playBehavior: PlayBehavior)

    fun selectThemeMode(themeMode: ThemeMode)

    fun selectColorPalette(colorPalette: ColorPalette)

    fun selectFontFamily(fontFamily: AppFontFamily)

    fun setCacheEnabled(enabled: Boolean)
}
