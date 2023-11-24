package com.lelloman.pezzottify.android.app.ui.splash

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.domain.LoginManager
import com.lelloman.pezzottify.android.app.domain.LoginState
import com.lelloman.pezzottify.android.app.ui.Navigator
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SplashViewModel @Inject constructor(
    private val loginManager: LoginManager,
    private val navigator: Navigator,
) : ViewModel() {

    fun onShown() = viewModelScope.launch {
        when (loginManager.loginState.filter { it !is LoginState.Loading }.first()) {
            is LoginState.LoggedIn -> navigator.fromSplashToHome()
            is LoginState.Unauthenticated -> navigator.fromSplashToLogin()
            else -> {}
        }
    }
}