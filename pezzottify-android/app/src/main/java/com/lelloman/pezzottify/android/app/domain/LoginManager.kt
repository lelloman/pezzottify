package com.lelloman.pezzottify.android.app.domain

import com.lelloman.pezzottify.remoteapi.LoginResponse
import com.lelloman.pezzottify.remoteapi.RemoteApi
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import java.io.File
import java.net.ConnectException

interface LoginManager {

    val loginState: Flow<LoginState>

    suspend fun performLogin(remoteUrl: String, username: String, password: String): LoginResult

    suspend fun logout()
}

class LoginManagerImpl(
    private val remoteApi: RemoteApi,
    private val persistence: File,
    private val ioDispatcher: CoroutineDispatcher,
    private val loginOperations: Set<LoginOperation>,
    private val logoutOperations: Set<LogoutOperation>,
) : LoginManager {

    private val stateBroadcast = MutableStateFlow<LoginState>(LoginState.Loading)

    override val loginState: StateFlow<LoginState> by lazy {
        loadPersistedState()
        stateBroadcast
    }

    private fun loadPersistedState() {
        val state = try {
            val (remoteUrl, username, authToken) = persistence.readText()
                .lines()
                .takeIf { it.size > 2 }
                ?.take(3)
                ?.takeIf { lines -> lines.all { it.isNotBlank() } }
                ?: throw Exception()

            LoginState.LoggedIn(
                username = username,
                authToken = authToken,
                remoteUrl = remoteUrl,
            )

        } catch (_: Throwable) {
            LoginState.Unauthenticated
        }
        runBlocking { stateBroadcast.emit(state) }
    }

    override suspend fun logout() = withContext(ioDispatcher) {
        logoutOperations.forEach { operation ->
            try {
                operation()
            } catch (e: Throwable) {
                e.printStackTrace()
            }
        }
        persistence.delete()
        stateBroadcast.emit(LoginState.Unauthenticated)
    }

    private suspend fun handleSuccessfulLogin(state: LoginState.LoggedIn): Boolean {
        if (loginOperations.any { it(state).not() }) return false
        stateBroadcast.emit(state as LoginState)
        persistence.writeText("${state.remoteUrl}\n${state.username}\n${state.authToken}")
        return true
    }

    override suspend fun performLogin(
        remoteUrl: String,
        username: String,
        password: String
    ): LoginResult {
        return withContext(ioDispatcher) {
            try {
                when (val response =
                    remoteApi.performLogin(
                        username = username,
                        password = password,
                        remoteUrl = remoteUrl
                    )) {
                    is RemoteApi.Response.Success -> {
                        when (response.value) {
                            is LoginResponse.Success -> {
                                val token = (response.value as LoginResponse.Success).authToken
                                val loggedInState = LoginState.LoggedIn(
                                    username = username,
                                    authToken = token,
                                    remoteUrl = remoteUrl,
                                )
                                if (handleSuccessfulLogin(loggedInState)) {
                                    LoginResult.Success(token)
                                } else {
                                    LoginResult.Failure.Unknown
                                }
                            }

                            is LoginResponse.InvalidCredentials -> {
                                LoginResult.Failure.Credentials
                            }
                        }
                    }

                    is RemoteApi.Response.NetworkError -> LoginResult.Failure.Network
                    else -> LoginResult.Failure.Unknown
                }
            } catch (e: ConnectException) {
                LoginResult.Failure.Network
            } catch (e: Exception) {
                when {
                    e.message?.contains("network") == true -> LoginResult.Failure.Network
                    e.message?.contains("credentials") == true -> LoginResult.Failure.Credentials
                    else -> LoginResult.Failure.Unknown
                }
            }
        }
    }
}