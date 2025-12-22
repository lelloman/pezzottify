package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.runBlocking
import okhttp3.Interceptor
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import java.util.concurrent.atomic.AtomicLong

/**
 * OkHttp interceptor that handles session expiration (401/403 responses)
 * by attempting to refresh the token before triggering logout.
 *
 * Also handles 429 (Too Many Requests) responses with exponential backoff.
 */
internal class SessionExpiredInterceptor(
    private val sessionExpiredHandler: SessionExpiredHandler,
    private val tokenRefresher: TokenRefresher,
    private val logger: Logger,
) : Interceptor {

    // Track when we're rate limited until (0 = not rate limited)
    private val rateLimitedUntil = AtomicLong(0)

    override fun intercept(chain: Interceptor.Chain): Response {
        val request = chain.request()

        // Skip auth endpoints to avoid loops
        val path = request.url.encodedPath
        if (isAuthEndpoint(path)) {
            return chain.proceed(request)
        }

        // Check if we're currently in a rate-limit backoff period
        val backoffUntil = rateLimitedUntil.get()
        if (backoffUntil > 0) {
            val remainingMs = backoffUntil - System.currentTimeMillis()
            if (remainingMs > 0) {
                logger.warn("Request blocked due to rate limit backoff (${remainingMs}ms remaining)")
                // Return a synthetic 429 response instead of making the request
                return Response.Builder()
                    .request(request)
                    .protocol(okhttp3.Protocol.HTTP_1_1)
                    .code(429)
                    .message("Too Many Requests (client-side backoff)")
                    .body("Rate limited - retry after ${remainingMs}ms".toResponseBody(null))
                    .build()
            } else {
                // Backoff period has passed, clear the flag
                rateLimitedUntil.compareAndSet(backoffUntil, 0)
            }
        }

        val response = chain.proceed(request)

        // Handle 429 from the server
        if (response.code == 429) {
            val retryAfterMs = parseRetryAfter(response)
            logger.warn("Received 429 from server, backing off for ${retryAfterMs}ms")
            rateLimitedUntil.set(System.currentTimeMillis() + retryAfterMs)
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

                is TokenRefresher.RefreshResult.RateLimited -> {
                    logger.warn("Token refresh rate limited, backing off for ${refreshResult.retryAfterMs}ms")
                    rateLimitedUntil.set(System.currentTimeMillis() + refreshResult.retryAfterMs)
                    // Return the original 401/403 response - client will need to retry later
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

    /**
     * Parse the Retry-After header from a 429 response.
     * Returns the backoff time in milliseconds.
     */
    private fun parseRetryAfter(response: Response): Long {
        val retryAfter = response.header("Retry-After")
        if (retryAfter != null) {
            // Try to parse as seconds (integer)
            retryAfter.toLongOrNull()?.let { seconds ->
                return seconds * 1000
            }
            // Could also parse HTTP-date format, but most servers use seconds
        }
        return DEFAULT_BACKOFF_MS
    }

    companion object {
        private const val DEFAULT_BACKOFF_MS = 60_000L // 1 minute default
    }
}
