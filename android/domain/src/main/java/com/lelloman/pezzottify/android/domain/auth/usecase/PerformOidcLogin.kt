package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Completes OIDC login after receiving authorization callback.
 */
class PerformOidcLogin @Inject constructor(
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
    private val syncManager: SyncManager,
    private val webSocketManager: WebSocketManager,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Complete OIDC login with authorization result.
     */
    suspend operator fun invoke(result: OidcAuthManager.AuthorizationResult): LoginResult {
        return when (result) {
            is OidcAuthManager.AuthorizationResult.Success -> {
                logger.info("invoke() OIDC login successful for user: ${result.userHandle}")
                authStore.storeAuthState(
                    AuthState.LoggedIn(
                        userHandle = result.userHandle,
                        authToken = result.idToken,
                        refreshToken = result.refreshToken,
                        remoteUrl = configStore.baseUrl.value,
                    )
                )
                logger.debug("invoke() connecting WebSocket")
                webSocketManager.connect()
                logger.debug("invoke() initializing sync manager")
                syncManager.initialize()
                LoginResult.Success
            }

            is OidcAuthManager.AuthorizationResult.Cancelled -> {
                logger.info("invoke() OIDC login cancelled by user")
                LoginResult.Cancelled
            }

            is OidcAuthManager.AuthorizationResult.Error -> {
                logger.error("invoke() OIDC login failed: ${result.message}")
                LoginResult.Error(result.message)
            }
        }
    }

    sealed interface LoginResult {
        data object Success : LoginResult
        data object Cancelled : LoginResult
        data class Error(val message: String) : LoginResult
    }
}
