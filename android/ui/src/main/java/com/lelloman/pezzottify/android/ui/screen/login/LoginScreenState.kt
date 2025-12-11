package com.lelloman.pezzottify.android.ui.screen.login

import androidx.annotation.StringRes

data class LoginScreenState(
    val host: String = "",
    val email: String = "",
    val password: String = "",
    val isLoading: Boolean = false,
    @StringRes val hostErrorRes: Int? = null,
    @StringRes val emailErrorRes: Int? = null,
    @StringRes val errorRes: Int? = null,
)