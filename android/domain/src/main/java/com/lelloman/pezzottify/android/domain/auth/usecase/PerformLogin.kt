package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import javax.inject.Inject

class PerformLogin @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
    private val syncManager: SyncManager,
    private val deviceInfoProvider: DeviceInfoProvider,
    private val webSocketManager: WebSocketManager,
) : UseCase() {

    suspend operator fun invoke(email: String, password: String): LoginResult {
        authStore.storeLastUsedCredentials(
            handle = email,
            baseUrl = configStore.baseUrl.value,
        )
        val deviceInfo = deviceInfoProvider.getDeviceInfo()
        when (val remoteResponse = remoteApiClient.login(email, password, deviceInfo)) {
            is RemoteApiResponse.Success -> {
                authStore.storeAuthState(
                    AuthState.LoggedIn(
                        userHandle = email,
                        remoteUrl = configStore.baseUrl.value,
                        authToken = remoteResponse.data.token,
                    )
                )
                webSocketManager.connect()
                syncManager.initialize()
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