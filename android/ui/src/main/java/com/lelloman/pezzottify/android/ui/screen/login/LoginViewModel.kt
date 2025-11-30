package com.lelloman.pezzottify.android.ui.screen.login

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class LoginViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), LoginScreenActions {

    private val mutableState: MutableStateFlow<LoginScreenState> =
        MutableStateFlow(
            LoginScreenState(
                host = interactor.getInitialHost(),
                email = interactor.getInitialEmail(),
            )
        )
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<LoginScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    override fun updateHost(host: String) {
        mutableState.value = mutableState.value.copy(host = host)
        viewModelScope.launch {
            interactor.setHost(host)
        }
    }

    override fun updateEmail(email: String) {
        mutableState.value = mutableState.value.copy(email = email)
    }

    override fun updatePassword(password: String) {
        mutableState.value = mutableState.value.copy(password = password)
    }

    override fun clockOnLoginButton() {
        if (!mutableState.value.isLoading) {
            mutableState.value = mutableState.value.copy(isLoading = true)
            viewModelScope.launch {
                val loginResult = interactor.login(
                    email = mutableState.value.email,
                    password = mutableState.value.password,
                )
                when (loginResult) {
                    is Interactor.LoginResult.Success -> {
                        mutableEvents.emit(LoginScreenEvents.NavigateToMain)
                    }
                    is Interactor.LoginResult.Failure.InvalidCredentials -> {
                        mutableState.value = mutableState.value.copy(
                            error = "Invalid credentials",
                        )
                    }
                    is Interactor.LoginResult.Failure.Unknown -> {
                        mutableState.value = mutableState.value.copy(
                            error = "Unknown error",
                        )
                    }
                }
                mutableState.value = mutableState.value.copy(isLoading = false)
            }
        }
    }

    interface Interactor {

        fun getInitialHost(): String

        fun getInitialEmail(): String

        suspend fun setHost(host: String)

        suspend fun login(email: String, password: String): LoginResult

        sealed interface LoginResult {

            data object Success : LoginResult

            sealed interface Failure : LoginResult {
                data object InvalidCredentials : Failure
                data object Unknown : Failure
            }
        }
    }
}