package com.lelloman.pezzottify.android.domain.auth

import kotlinx.serialization.Serializable

sealed interface AuthState {

    data object Loading : AuthState

    data object LoggedOut : AuthState

    @Serializable
    data class LoggedIn(
        val userHandle: String,
        val authToken: String,
        val refreshToken: String? = null,
        val remoteUrl: String,
    ) : AuthState
}