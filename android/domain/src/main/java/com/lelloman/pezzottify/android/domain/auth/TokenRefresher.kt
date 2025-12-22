package com.lelloman.pezzottify.android.domain.auth

/**
 * Interface for refreshing authentication tokens.
 * Used by the network layer to transparently refresh expired tokens.
 */
interface TokenRefresher {

    /**
     * Attempts to refresh the current authentication tokens.
     *
     * @return RefreshResult indicating success or failure
     */
    suspend fun refreshTokens(): RefreshResult

    /**
     * Result of a token refresh attempt.
     */
    sealed interface RefreshResult {
        /**
         * Token refresh successful. The new auth token is returned.
         */
        data class Success(val newAuthToken: String) : RefreshResult

        /**
         * Token refresh failed. User needs to re-authenticate.
         */
        data class Failed(val reason: String) : RefreshResult

        /**
         * No refresh token available (legacy auth or not logged in).
         */
        data object NotAvailable : RefreshResult

        /**
         * Token refresh was rate limited by the OIDC provider.
         * Caller should implement backoff before retrying.
         *
         * @param retryAfterMs Suggested delay before retrying, in milliseconds.
         *                     If the server provided a Retry-After header, this reflects that value.
         *                     Otherwise, it's a default backoff suggestion.
         */
        data class RateLimited(val retryAfterMs: Long) : RefreshResult
    }
}
