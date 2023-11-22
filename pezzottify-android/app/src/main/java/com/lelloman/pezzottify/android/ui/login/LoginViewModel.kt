package com.lelloman.pezzottify.android.ui.login

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.LoginManager
import com.lelloman.pezzottify.android.domain.LoginResult
import com.lelloman.pezzottify.android.ui.Navigator
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

@HiltViewModel
class LoginViewModel @Inject constructor(
    private val loginManager: LoginManager,
    private val navigator: Navigator,
) : ViewModel() {

    private val mutableState = MutableStateFlow(State())
    val state: StateFlow<State> = mutableState.asStateFlow()

    fun onLoginClicked() = viewModelScope.launch {
        state.value.let { currentState ->
            if (currentState.loading) return@launch
            updateState(mutableState.value.copy(loading = true))
            val loginResult = loginManager.performLogin(
                username = currentState.username,
                password = currentState.password
            )
            when (loginResult) {
                is LoginResult.Success -> navigator.fromLoginToHome()
                is LoginResult.Failure -> {}
            }
            updateState(mutableState.value.copy(loading = false))
        }
    }

    fun onUsernameUpdate(newUsername: String) {
        updateState(state.value.copy(username = newUsername))
    }

    fun onPasswordUpdate(newPassword: String) {
        updateState(state.value.copy(password = newPassword))
    }

    private fun updateState(newState: State) = runBlocking {
        mutableState.emit(newState)
    }

    data class State(
        val username: String = "",
        val password: String = "",
        val loading: Boolean = false,
    )
}