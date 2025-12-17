package com.lelloman.pezzottify.android.ui.screen.login

sealed interface LoginScreenEvents {
    data object NavigateToMain : LoginScreenEvents
    data object RequestNotificationPermission : LoginScreenEvents
}