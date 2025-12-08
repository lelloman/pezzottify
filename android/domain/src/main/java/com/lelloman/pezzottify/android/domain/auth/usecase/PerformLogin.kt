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
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

class PerformLogin @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
    private val syncManager: SyncManager,
    private val deviceInfoProvider: DeviceInfoProvider,
    private val webSocketManager: WebSocketManager,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    suspend operator fun invoke(email: String, password: String): LoginResult {
        logger.info("invoke() attempting login for user: $email")
        authStore.storeLastUsedCredentials(
            handle = email,
            baseUrl = configStore.baseUrl.value,
        )
        val deviceInfo = deviceInfoProvider.getDeviceInfo()
        logger.debug("invoke() device info: $deviceInfo")
        when (val remoteResponse = remoteApiClient.login(email, password, deviceInfo)) {
            is RemoteApiResponse.Success -> {
                logger.info("invoke() login successful for user: $email")
                authStore.storeAuthState(
                    AuthState.LoggedIn(
                        userHandle = email,
                        remoteUrl = configStore.baseUrl.value,
                        authToken = remoteResponse.data.token,
                    )
                )
                logger.debug("invoke() connecting WebSocket")
                webSocketManager.connect()
                logger.debug("invoke() initializing sync manager")
                syncManager.initialize()
                return LoginResult.Success
            }

            RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("invoke() login failed - wrong credentials for user: $email")
                return LoginResult.WrongCredentials
            }
            else -> {
                logger.error("invoke() login failed with error: $remoteResponse")
                return LoginResult.Error
            }
        }

    }

    enum class LoginResult {
        Success,
        WrongCredentials,
        Error,
    }
}