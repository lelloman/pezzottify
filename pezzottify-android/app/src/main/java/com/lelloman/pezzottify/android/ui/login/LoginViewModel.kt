package com.lelloman.pezzottify.android.ui.login

import androidx.lifecycle.ViewModel
import com.lelloman.pezzottify.android.domain.LoginManager
import com.lelloman.pezzottify.android.domain.MockLoginManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import javax.inject.Inject

@HiltViewModel
class LoginViewModel @Inject constructor(
    private val loginManager: LoginManager,
) : ViewModel() {

    private val mutableState = MutableStateFlow(State())
    val state = mutableState.asStateFlow()

    suspend fun onLoginClicked() {
        state.value.let { currentState ->
            if (currentState.loading) return
            updateState(mutableState.value.copy(loading = true))
            withContext(Dispatchers.IO) {
                val loginResult = loginManager.performLogin(
                    username = currentState.username,
                    password = currentState.password
                )
            }
            updateState(mutableState.value.copy(loading = false))
        }
    }

    private fun updateState(newState: State) {

    }

    data class State(
        val username: String = "",
        val password: String = "",
        val loading: Boolean = false,
    )
}