package com.lelloman.pezzottify.android.ui.login

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.LoginManager
import com.lelloman.pezzottify.android.domain.LoginResult
import com.lelloman.pezzottify.android.persistence.ObjectsStore
import com.lelloman.pezzottify.android.persistence.PersistentObjectDef
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
    private val objectsStore: ObjectsStore,
) : ViewModel() {

    private val mutableState by lazy {
        loadPersistentObject()
        MutableStateFlow(State())
    }
    val state: StateFlow<State> = mutableState.asStateFlow()

    fun onLoginClicked() = viewModelScope.launch {
        state.value.let { currentState ->
            if (currentState.loading) return@launch
            updateState { it.copy(loading = true) }
            val loginResult = loginManager.performLogin(
                username = currentState.username,
                password = currentState.password,
                remoteUrl = currentState.remoteUrl,
            )
            when (loginResult) {
                is LoginResult.Success -> navigator.fromLoginToHome()
                is LoginResult.Failure -> {}
            }
            updateState { it.copy(loading = false) }
            objectsStore.store(
                PersistentObject.def.key,
                PersistentObject(
                    username = currentState.username,
                    remoteUrl = currentState.remoteUrl
                )
            )
        }
    }

    private fun loadPersistentObject() {
        viewModelScope.launch {
            try {
                val obj: PersistentObject = objectsStore.load(PersistentObject.def.key)
                updateState { currentState ->
                    currentState.copy(username = obj.username, remoteUrl = obj.remoteUrl)
                }
            } catch (_: Throwable) {
            }
        }
    }

    fun onRemoteUrlUpdate(remoteUrl: String) = updateState { it.copy(remoteUrl = remoteUrl) }

    fun onUsernameUpdate(newUsername: String) = updateState { it.copy(username = newUsername) }

    fun onPasswordUpdate(newPassword: String) = updateState { it.copy(password = newPassword) }

    private fun updateState(action: (State) -> State) = runBlocking {
        val currentState = state.value
        val newState = action(currentState)
        mutableState.emit(newState)
    }

    data class PersistentObject(
        val username: String,
        val remoteUrl: String,
    ) {
        companion object {
            val def = PersistentObjectDef("Login", PersistentObject::class)
        }
    }

    data class State(
        val username: String = "",
        val password: String = "",
        val remoteUrl: String = "",
        val loading: Boolean = false,
    )
}