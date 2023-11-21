package com.lelloman.pezzottify.android.ui.login

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.LoginManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class LoginViewModel @Inject constructor(
    private val loginManager: LoginManager,
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
            updateState(mutableState.value.copy(loading = false))
        }
    }

    private suspend fun updateState(newState: State) {
        mutableState.emit(newState)
    }

    data class State(
        val username: String = "",
        val password: String = "",
        val loading: Boolean = false,
    )
}