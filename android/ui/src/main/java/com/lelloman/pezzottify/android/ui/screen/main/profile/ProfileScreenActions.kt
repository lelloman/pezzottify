package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode

interface ProfileScreenActions {

    fun clickOnLogout()

    fun confirmLogout()

    fun dismissLogoutConfirmation()

    fun selectPlayBehavior(playBehavior: PlayBehavior)

    fun selectThemeMode(themeMode: ThemeMode)
}
