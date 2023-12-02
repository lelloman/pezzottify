package com.lelloman.pezzottify.android.app.ui.login

import android.util.Patterns
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.login.LoginResult
import com.lelloman.pezzottify.android.app.localdata.ObjectsStore
import com.lelloman.pezzottify.android.app.localdata.PersistentObjectDef
import com.lelloman.pezzottify.android.app.ui.Navigator
import com.lelloman.pezzottify.android.app.ui.SnackBarController
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
    private val snackBarController: SnackBarController,
    private val objectsStore: ObjectsStore,
) : ViewModel() {

    private val mutableState by lazy {
        loadPersistentObject()
        MutableStateFlow(State())
    }
    val state: StateFlow<State> = mutableState.asStateFlow()

    private val State.hasInvalidUrl
        get() = try {
            Patterns.WEB_URL.matcher(remoteUrl.value).matches().not()
        } catch (_: Throwable) {
            true
        }

    fun onLoginClicked() = viewModelScope.launch {
        state.value.let { currentState ->
            if (currentState.loading) return@launch

            if (currentState.hasInvalidUrl) {
                updateState { it.copy(remoteUrl = it.remoteUrl.copy(error = "Invalid url")) }
                return@launch
            }

            updateState { it.copy(loading = true) }
            val loginResult = loginManager.performLogin(
                username = currentState.username.value,
                password = currentState.password.value,
                remoteUrl = currentState.remoteUrl.value,
            )
            when (loginResult) {
                is LoginResult.Success -> navigator.fromLoginToHome()
                is LoginResult.Failure.Unknown -> snackBarController.showSnack("Something went wrong...")
                is LoginResult.Failure.Credentials -> {
                    updateState { prevState ->
                        prevState.copy(
                            username = prevState.username.copy(error = "Wrong credentials"),
                            password = prevState.password.copy(error = "Wrong credentials"),
                        )
                    }
                }
                is LoginResult.Failure.Network -> snackBarController.showSnack("Connection error")
            }
            updateState { it.copy(loading = false) }
            objectsStore.store(
                PersistentObject.def, PersistentObject(
                    username = currentState.username.value, remoteUrl = currentState.remoteUrl.value
                )
            )
        }
    }

    private fun loadPersistentObject() {
        viewModelScope.launch {
            try {
                val obj: PersistentObject = objectsStore.load(PersistentObject.def)
                updateState { currentState ->
                    currentState.copy(
                        username = TextField(obj.username),
                        remoteUrl = TextField(obj.remoteUrl),
                    )
                }
            } catch (_: Throwable) {
            }
        }
    }

    fun onRemoteUrlUpdate(remoteUrl: String) =
        updateState { it.copy(remoteUrl = TextField(remoteUrl)) }

    fun onUsernameUpdate(newUsername: String) =
        updateState { it.copy(username = TextField(newUsername)) }

    fun onPasswordUpdate(newPassword: String) =
        updateState { it.copy(password = TextField(newPassword)) }

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
        val username: TextField = TextField(),
        val password: TextField = TextField(),
        val remoteUrl: TextField = TextField(),
        val loading: Boolean = false,
    )

    data class TextField(
        val value: String = "",
        val error: String? = null,
    ) {
        val hasError = error != null
    }
}