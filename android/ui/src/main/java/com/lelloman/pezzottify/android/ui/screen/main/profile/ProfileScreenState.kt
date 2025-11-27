package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode

data class ProfileScreenState(
    val userName: String = "",
    val baseUrl: String = "",
    val playBehavior: PlayBehavior = PlayBehavior.Default,
    val themeMode: ThemeMode = ThemeMode.Default,
    val isLoggingOut: Boolean = false,
)
