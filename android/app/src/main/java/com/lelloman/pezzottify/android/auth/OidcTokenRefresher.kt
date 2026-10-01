package com.lelloman.pezzottify.android.auth

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.cancellation.CancellationException

/**
 * Token refresher that coalesces concurrent refresh requests.
 *
 * When multiple concurrent requests receive 401 responses, they all call refreshTokens().
 * This implementation ensures only ONE actual OIDC refresh is performed, and all callers
 * share the same result. This prevents rate limiting issues with the OIDC provider.
 */
@Singleton
class OidcTokenRefresher @Inject constructor(
    private val authStore: AuthStore,
    private val oidcAuthManager: OidcAuthManager,
    loggerFactory: LoggerFactory,
) : TokenRefresher {

    private val logger: Logger by loggerFactory
    private val mutex = Mutex()

    // Tracks the in-flight refresh operation. All concurrent callers await this same Deferred.
    private var inFlightRefresh: CompletableDeferred<TokenRefresher.RefreshResult>? = null

    override suspend fun refreshTokens(): TokenRefresher.RefreshResult {
        // Check if there's already an in-flight refresh we can join
        val existingOrNew = mutex.withLock {
            inFlightRefresh?.let { existing ->
                logger.debug("refreshTokens() joining existing in-flight refresh")
                return@withLock existing to false
            }

            // No in-flight refresh - we'll be the one to perform it
            val deferred = CompletableDeferred<TokenRefresher.RefreshResult>()
            inFlightRefresh = deferred
            deferred to true
        }

        val (deferred, isOwner) = existingOrNew

        if (!isOwner) {
            // We're not the owner - just await the result
            return deferred.await()
        }

        // We're the owner - perform the refresh
        // Use try-finally to ensure we always complete the deferred and clear state
        var result: TokenRefresher.RefreshResult? = null
        try {
            // A refresh grant may rotate the credential even if its caller is cancelled.
            // Finish the exchange and persist its result before honouring cancellation.
            // Complete waiters inside this context: withContext can throw cancellation
            // while returning to the owner after the rotated token has been saved.
            withContext(NonCancellable) {
                val completedResult = performRefresh()
                result = completedResult
                deferred.complete(completedResult)
            }
        } catch (e: CancellationException) {
            // Propagate cancellation, but still complete deferred for other waiters
            if (result == null) result = TokenRefresher.RefreshResult.Failed("Cancelled")
            throw e
        } catch (e: Exception) {
            logger.error("refreshTokens() unexpected error", e)
            result = TokenRefresher.RefreshResult.Failed("Unexpected error: ${e.message}")
        } finally {
            // Always complete the deferred and clear state
            deferred.complete(result ?: TokenRefresher.RefreshResult.Failed("Unknown error"))
            withContext(NonCancellable) {
                mutex.withLock {
                    inFlightRefresh = null
                }
            }
        }

        return result ?: TokenRefresher.RefreshResult.Failed("Unknown error")
    }

    private suspend fun performRefresh(): TokenRefresher.RefreshResult {
        logger.debug("refreshTokens() starting actual refresh")

        val currentState = authStore.getAuthState().value
        if (currentState !is AuthState.LoggedIn) {
            logger.debug("refreshTokens() not logged in")
            return TokenRefresher.RefreshResult.NotAvailable
        }

        val refreshToken = currentState.refreshToken
        if (refreshToken == null) {
            logger.debug("refreshTokens() no refresh token available (legacy auth?)")
            return TokenRefresher.RefreshResult.NotAvailable
        }

        logger.debug("refreshTokens() attempting OIDC token refresh")
        val oidcResult = oidcAuthManager.refreshTokens(refreshToken)
        // Neither success nor rejection from an old session applies to a new sign-in.
        if (authStore.getAuthState().value != currentState) {
            return TokenRefresher.RefreshResult.Failed("Session changed during refresh")
        }
        return when (oidcResult) {
            is OidcAuthManager.RefreshResult.Success -> {
                val newAuthToken = oidcResult.idToken
                if (newAuthToken.isNullOrBlank()) {
                    // The API authenticates with the ID token. Retrying with the token that just
                    // received a 401 would report a false refresh success and leave the app stuck.
                    // A provider may rotate the refresh token even in an incomplete response.
                    // Keep it so the next attempt does not reuse a revoked credential.
                    oidcResult.refreshToken?.let {
                        authStore.storeAuthState(currentState.copy(refreshToken = it))
                    }
                    logger.warn("refreshTokens() response did not include a fresh ID token")
                    return TokenRefresher.RefreshResult.Failed("No fresh ID token received")
                }
                logger.info("refreshTokens() success")
                val newState = currentState.copy(
                    authToken = newAuthToken,
                    refreshToken = oidcResult.refreshToken ?: currentState.refreshToken,
                )
                authStore.storeAuthState(newState)
                TokenRefresher.RefreshResult.Success(newAuthToken)
            }

            is OidcAuthManager.RefreshResult.Failed -> {
                logger.warn("refreshTokens() failed: ${oidcResult.message}")
                TokenRefresher.RefreshResult.Failed(oidcResult.message, oidcResult.requiresReauthentication)
            }

            is OidcAuthManager.RefreshResult.RateLimited -> {
                logger.warn("refreshTokens() rate limited, retryAfterMs=${oidcResult.retryAfterMs}")
                TokenRefresher.RefreshResult.RateLimited(oidcResult.retryAfterMs)
            }
        }
    }
}
