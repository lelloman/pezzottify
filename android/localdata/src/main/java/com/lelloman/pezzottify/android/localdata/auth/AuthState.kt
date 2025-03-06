package com.lelloman.pezzottify.android.localdata.auth

sealed interface AuthState {

    data object LoggedOut : AuthState

    data class LoggedIn(
        val userHandle: String,
        val authToken: String,
        val remoteUrl: String,
    )
}