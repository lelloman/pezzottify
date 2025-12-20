package com.lelloman.pezzottify.android.ui.screen.login

import android.content.Intent
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
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

    init {
        // Collect OIDC callback intents
        viewModelScope.launch {
            interactor.oidcCallbacks().collect { callbackIntent ->
                handleOidcCallback(callbackIntent)
            }
        }
    }

    override fun updateHost(host: String) {
        mutableState.value = mutableState.value.copy(host = host, hostErrorRes = null)
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
                val hostResult = interactor.setHost(mutableState.value.host)
                if (hostResult is Interactor.SetHostResult.InvalidUrl) {
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        hostErrorRes = R.string.invalid_url,
                    )
                    return@launch
                }

                val loginResult = interactor.login(
                    email = mutableState.value.email,
                    password = mutableState.value.password,
                )
                when (loginResult) {
                    is Interactor.LoginResult.Success -> {
                        mutableEvents.emit(LoginScreenEvents.RequestNotificationPermission)
                        mutableEvents.emit(LoginScreenEvents.NavigateToMain)
                    }
                    is Interactor.LoginResult.Failure.InvalidCredentials -> {
                        mutableState.value = mutableState.value.copy(
                            errorRes = R.string.invalid_credentials,
                        )
                    }
                    is Interactor.LoginResult.Failure.Unknown -> {
                        mutableState.value = mutableState.value.copy(
                            errorRes = R.string.unknown_error,
                        )
                    }
                }
                mutableState.value = mutableState.value.copy(isLoading = false)
            }
        }
    }

    override fun clickOnOidcLogin() {
        if (!mutableState.value.isLoading) {
            mutableState.value = mutableState.value.copy(isLoading = true)
            viewModelScope.launch {
                val hostResult = interactor.setHost(mutableState.value.host)
                if (hostResult is Interactor.SetHostResult.InvalidUrl) {
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        hostErrorRes = R.string.invalid_url,
                    )
                    return@launch
                }

                when (val result = interactor.createOidcAuthIntent()) {
                    is Interactor.OidcIntentResult.Success -> {
                        mutableEvents.emit(LoginScreenEvents.LaunchOidcIntent(result.intent))
                    }
                    is Interactor.OidcIntentResult.Error -> {
                        mutableState.value = mutableState.value.copy(isLoading = false)
                        mutableEvents.emit(LoginScreenEvents.OidcError(result.message))
                    }
                }
            }
        }
    }

    private suspend fun handleOidcCallback(callbackIntent: Intent) {
        when (val result = interactor.handleOidcCallback(callbackIntent)) {
            is Interactor.OidcLoginResult.Success -> {
                mutableState.value = mutableState.value.copy(isLoading = false)
                mutableEvents.emit(LoginScreenEvents.RequestNotificationPermission)
                mutableEvents.emit(LoginScreenEvents.NavigateToMain)
            }
            is Interactor.OidcLoginResult.Cancelled -> {
                mutableState.value = mutableState.value.copy(isLoading = false)
            }
            is Interactor.OidcLoginResult.Error -> {
                mutableState.value = mutableState.value.copy(isLoading = false)
                mutableEvents.emit(LoginScreenEvents.OidcError(result.message))
            }
        }
    }

    interface Interactor {

        fun getInitialHost(): String

        fun getInitialEmail(): String

        suspend fun setHost(host: String): SetHostResult

        suspend fun login(email: String, password: String): LoginResult

        fun oidcCallbacks(): SharedFlow<Intent>

        suspend fun createOidcAuthIntent(): OidcIntentResult

        suspend fun handleOidcCallback(intent: Intent): OidcLoginResult

        sealed interface SetHostResult {
            data object Success : SetHostResult
            data object InvalidUrl : SetHostResult
        }

        sealed interface LoginResult {

            data object Success : LoginResult

            sealed interface Failure : LoginResult {
                data object InvalidCredentials : Failure
                data object Unknown : Failure
            }
        }

        sealed interface OidcIntentResult {
            data class Success(val intent: Intent) : OidcIntentResult
            data class Error(val message: String) : OidcIntentResult
        }

        sealed interface OidcLoginResult {
            data object Success : OidcLoginResult
            data object Cancelled : OidcLoginResult
            data class Error(val message: String) : OidcLoginResult
        }
    }
}