package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode

data class ProfileScreenState(
    val userName: String = "",
    val baseUrl: String = "",
    val playBehavior: PlayBehavior = PlayBehavior.Default,
    val themeMode: ThemeMode = ThemeMode.Default,
    val colorPalette: ColorPalette = ColorPalette.Default,
    val fontFamily: AppFontFamily = AppFontFamily.Default,
    val isLoggingOut: Boolean = false,
    val showLogoutConfirmation: Boolean = false,
    val buildVariant: String = "",
    val versionName: String = "",
    val gitCommit: String = "",
)
