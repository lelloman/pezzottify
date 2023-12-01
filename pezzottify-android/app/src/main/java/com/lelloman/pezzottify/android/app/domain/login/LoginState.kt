package com.lelloman.pezzottify.android.app.domain.login

sealed class LoginState {
    object Loading : LoginState()
    object Unauthenticated : LoginState()
    data class LoggedIn(
        val username: String,
        val authToken: String,
        val remoteUrl: String,
    ) : LoginState()
}