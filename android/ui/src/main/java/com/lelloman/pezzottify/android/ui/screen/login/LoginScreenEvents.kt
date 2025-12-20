package com.lelloman.pezzottify.android.ui.screen.login

import android.content.Intent

sealed interface LoginScreenEvents {
    data object NavigateToMain : LoginScreenEvents
    data object RequestNotificationPermission : LoginScreenEvents
    data class LaunchOidcIntent(val intent: Intent) : LoginScreenEvents
    data class OidcError(val message: String) : LoginScreenEvents
}