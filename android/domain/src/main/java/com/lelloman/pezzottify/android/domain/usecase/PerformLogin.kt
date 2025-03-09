package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import javax.inject.Inject

class PerformLogin @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
) : UseCase() {

    suspend operator fun invoke(email: String, password: String): LoginResult {
        when (val remoteResponse = remoteApiClient.login(email, password)) {
            is RemoteApiResponse.Success -> {
                authStore.storeAuthState(
                    AuthState.LoggedIn(
                        email,
                        configStore.baseUrl.value,
                        remoteResponse.data.token
                    )
                )
                return LoginResult.Success
            }

            RemoteApiResponse.Error.Unauthorized -> return LoginResult.WrongCredentials
            else -> return LoginResult.Error
        }

    }

    enum class LoginResult {
        Success,
        WrongCredentials,
        Error,
    }
}