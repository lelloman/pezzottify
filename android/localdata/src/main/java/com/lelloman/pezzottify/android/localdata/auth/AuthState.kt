package com.lelloman.pezzottify.android.localdata.auth

import kotlinx.serialization.Serializable

sealed interface AuthState {

    data object Loading : AuthState

    data object LoggedOut : AuthState

    @Serializable
    data class LoggedIn(
        val userHandle: String,
        val authToken: String,
        val remoteUrl: String,
    ) : AuthState
}