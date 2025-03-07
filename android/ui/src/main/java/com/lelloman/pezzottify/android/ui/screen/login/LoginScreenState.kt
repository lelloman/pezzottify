package com.lelloman.pezzottify.android.ui.screen.login

data class LoginScreenState(
    val host: String = "",
    val email: String = "",
    val password: String = "",
    val isLoading: Boolean = false,
    val hostError: String? = null,
    val emailError: String? = null,
    val error: String? = null,
)