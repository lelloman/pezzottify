package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.runBlocking
import okhttp3.Interceptor
import okhttp3.Response

/**
 * OkHttp interceptor that handles session expiration (401/403 responses)
 * by attempting to refresh the token before triggering logout.
 */
internal class SessionExpiredInterceptor(
    private val sessionExpiredHandler: SessionExpiredHandler,
    private val tokenRefresher: TokenRefresher,
    private val logger: Logger,
) : Interceptor {

    override fun intercept(chain: Interceptor.Chain): Response {
        val request = chain.request()
        val response = chain.proceed(request)

        // Skip auth endpoints to avoid loops
        val path = request.url.encodedPath
        if (isAuthEndpoint(path)) {
            return response
        }

        // Check for unauthorized responses
        if (response.code == 401 || response.code == 403) {
            logger.warn("Unauthorized response (${response.code}) for ${request.url}, attempting token refresh")

            // Attempt to refresh the token
            val refreshResult = runBlocking { tokenRefresher.refreshTokens() }

            when (refreshResult) {
                is TokenRefresher.RefreshResult.Success -> {
                    logger.info("Token refresh successful, retrying request")
                    // Close the old response before retrying
                    response.close()

                    // Retry the request with the new token
                    val newRequest = request.newBuilder()
                        .header("Authorization", refreshResult.newAuthToken)
                        .build()
                    return chain.proceed(newRequest)
                }

                is TokenRefresher.RefreshResult.Failed -> {
                    logger.warn("Token refresh failed: ${refreshResult.reason}, triggering logout")
                    sessionExpiredHandler.onSessionExpired()
                }

                is TokenRefresher.RefreshResult.NotAvailable -> {
                    logger.warn("No refresh token available, triggering logout")
                    sessionExpiredHandler.onSessionExpired()
                }
            }
        }

        return response
    }

    private fun isAuthEndpoint(path: String): Boolean {
        return path.contains("/auth/login") ||
            path.contains("/auth/logout") ||
            path.contains("/auth/challenge")
    }
}
