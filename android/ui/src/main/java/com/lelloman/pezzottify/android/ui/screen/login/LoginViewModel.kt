package com.lelloman.pezzottify.android.ui.screen.login

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

class LoginViewModel : ViewModel(), LoginScreenActions {

    private val mutableState: MutableStateFlow<LoginScreenState> =
        MutableStateFlow(LoginScreenState())
    val state = mutableState.asStateFlow()

    override fun updateHost(host: String) {
        TODO("Not yet implemented")
    }

    override fun updateEmail(email: String) {
        TODO("Not yet implemented")
    }

    override fun updatePassword(password: String) {
        TODO("Not yet implemented")
    }

    override fun clockOnLoginButton() {
        TODO("Not yet implemented")
    }
}