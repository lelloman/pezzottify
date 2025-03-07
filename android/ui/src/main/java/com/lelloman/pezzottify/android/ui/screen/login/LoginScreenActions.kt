package com.lelloman.pezzottify.android.ui.screen.login

import kotlinx.coroutines.flow.Flow

interface LoginScreenActions {

    fun updateHost(host: String)

    fun updateEmail(email: String)

    fun updatePassword(password: String)

    fun clockOnLoginButton()
}

