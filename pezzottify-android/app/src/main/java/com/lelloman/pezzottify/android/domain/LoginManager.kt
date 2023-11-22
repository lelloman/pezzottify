package com.lelloman.pezzottify.android.domain

import com.lelloman.pezzottify.remoteapi.LoginResponse
import com.lelloman.pezzottify.remoteapi.RemoteApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import java.io.File

interface LoginManager {

    val loginState: Flow<LoginState>

    suspend fun performLogin(username: String, password: String): LoginResult
}

class LoginManagerImpl(private val remoteApi: RemoteApi, private val persistence: File) :
    LoginManager {

    private val stateBroadcast = MutableStateFlow<LoginState>(LoginState.Loading)

    override val loginState: Flow<LoginState> by lazy {
        loadPersistedState()
        stateBroadcast
    }

    private fun loadPersistedState() {
        val state = try {
            val (l1, l2) = persistence.readText()
                .lines()
                .takeIf { it.size > 1 }
                ?.take(2)
                ?.takeIf { lines -> lines.all { it.isNotBlank() } }
                ?: throw Exception()

            LoginState.LoggedIn(
                username = l1,
                authToken = l2,
            )

        } catch (_: Throwable) {
            LoginState.Unauthenticated
        }
        runBlocking { stateBroadcast.emit(state) }
    }

    override suspend fun performLogin(username: String, password: String): LoginResult {
        Dispatchers.IO.run {
            return try {
                when (val response =
                    remoteApi.performLogin(username = username, password = password)) {
                    is RemoteApi.Response.Success -> {
                        when (response.value) {
                            is LoginResponse.Success -> {
                                val token = (response.value as LoginResponse.Success).authToken
                                stateBroadcast.emit(
                                    LoginState.LoggedIn(username, token) as LoginState
                                )
                                persistence.writeText("$username\n$token")
                                LoginResult.Success(token)
                            }

                            is LoginResponse.InvalidCredentials -> {
                                LoginResult.Failure.Credentials
                            }
                        }
                    }

                    is RemoteApi.Response.NetworkError -> LoginResult.Failure.Network
                    else -> LoginResult.Failure.Unknown
                }
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