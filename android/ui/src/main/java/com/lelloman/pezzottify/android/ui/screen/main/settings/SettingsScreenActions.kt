package com.lelloman.pezzottify.android.ui.screen.main.settings

import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

interface SettingsScreenActions {

    fun selectThemeMode(themeMode: ThemeMode)

    fun selectColorPalette(colorPalette: ColorPalette)

    fun selectFontFamily(fontFamily: AppFontFamily)

    fun setCacheEnabled(enabled: Boolean)

    fun setNotifyWhatsNewEnabled(enabled: Boolean)

    fun setFileLoggingEnabled(enabled: Boolean)

    fun shareLogs()

    fun clearLogs()

    fun onBaseUrlInputChanged(input: String)

    fun saveBaseUrl()

    fun forceSkeletonResync()
}
