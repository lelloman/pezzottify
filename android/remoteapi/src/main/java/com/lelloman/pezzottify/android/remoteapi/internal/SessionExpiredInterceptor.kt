package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.logger.Logger
import okhttp3.Interceptor
import okhttp3.Response

/**
 * OkHttp interceptor that detects session expiration (401/403 responses)
 * and triggers the session expired handler.
 *
 * Note: The handler implementation checks auth state before emitting events,
 * so multiple concurrent 401/403 responses will only trigger one logout.
 */
internal class SessionExpiredInterceptor(
    private val sessionExpiredHandler: SessionExpiredHandler,
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
            logger.warn("Session expired detected (${response.code}) for ${request.url}")
            sessionExpiredHandler.onSessionExpired()
        }

        return response
    }

    private fun isAuthEndpoint(path: String): Boolean {
        return path.contains("/auth/login") ||
            path.contains("/auth/logout") ||
            path.contains("/auth/challenge")
    }
}
