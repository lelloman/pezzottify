package com.lelloman.pezzottify.android.ui.screen.login

data class LoginScreenState(
    val host: String,
    val email: String,
    val password: String,
    val isLoading: Boolean,
    val hostError: String?,
    val emailError: String?,
    val error: String?,
)