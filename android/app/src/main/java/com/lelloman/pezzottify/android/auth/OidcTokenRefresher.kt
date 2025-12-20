package com.lelloman.pezzottify.android.auth

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class OidcTokenRefresher @Inject constructor(
    private val authStore: AuthStore,
    private val oidcAuthManager: OidcAuthManager,
    loggerFactory: LoggerFactory,
) : TokenRefresher {

    private val logger: Logger by loggerFactory
    private val refreshMutex = Mutex()

    override suspend fun refreshTokens(): TokenRefresher.RefreshResult {
        // Use mutex to prevent multiple concurrent refresh attempts
        return refreshMutex.withLock {
            logger.debug("refreshTokens() starting")

            val currentState = authStore.getAuthState().value
            if (currentState !is AuthState.LoggedIn) {
                logger.debug("refreshTokens() not logged in")
                return@withLock TokenRefresher.RefreshResult.NotAvailable
            }

            val refreshToken = currentState.refreshToken
            if (refreshToken == null) {
                logger.debug("refreshTokens() no refresh token available (legacy auth?)")
                return@withLock TokenRefresher.RefreshResult.NotAvailable
            }

            logger.debug("refreshTokens() attempting OIDC token refresh")
            when (val result = oidcAuthManager.refreshTokens(refreshToken)) {
                is OidcAuthManager.RefreshResult.Success -> {
                    logger.info("refreshTokens() success, updating auth state")
                    val newState = currentState.copy(
                        authToken = result.idToken,
                        refreshToken = result.refreshToken,
                    )
                    authStore.storeAuthState(newState)
                    TokenRefresher.RefreshResult.Success(result.idToken)
                }

                is OidcAuthManager.RefreshResult.Failed -> {
                    logger.warn("refreshTokens() failed: ${result.message}")
                    TokenRefresher.RefreshResult.Failed(result.message)
                }
            }
        }
    }
}
