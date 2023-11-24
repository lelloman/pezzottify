package com.lelloman.pezzottify.android.app.domain

fun interface LoginOperation {
    suspend operator fun invoke(loginState: LoginState.LoggedIn): Boolean
}

fun interface LogoutOperation {
    suspend operator fun invoke()
}