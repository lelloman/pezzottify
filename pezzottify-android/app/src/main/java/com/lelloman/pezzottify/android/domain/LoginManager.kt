package com.lelloman.pezzottify.android.domain

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow

interface LoginManager {

    val loginState: Flow<LoginState>

    suspend fun performLogin(username: String, password: String): LoginResult
}

interface RemoteLoginInteractor {
    suspend fun doLogin()
}

class MockLoginManager(private val remoteLoginInteractor: RemoteLoginInteractor) : LoginManager {

    private val stateBroadcast = MutableStateFlow<LoginState>(LoginState.Loading)

    override val loginState: Flow<LoginState> = stateBroadcast

    override suspend fun performLogin(username: String, password: String): LoginResult {
        Dispatchers.IO.run {
            return try {
                remoteLoginInteractor.doLogin()
                stateBroadcast.emit(LoginState.LoggedIn("asd", "asd") as LoginState)
                LoginResult.Success("asd")
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