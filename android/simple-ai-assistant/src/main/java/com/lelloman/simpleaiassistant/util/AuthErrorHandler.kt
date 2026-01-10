package com.lelloman.simpleaiassistant.util

/**
 * Handler for authentication errors that may occur during LLM requests.
 *
 * Implementations can attempt to refresh tokens and signal whether the
 * request should be retried.
 */
fun interface AuthErrorHandler {

    /**
     * Called when an authentication error occurs (e.g., 401).
     *
     * @param errorMessage The error message from the LLM provider
     * @return true if tokens were refreshed and the request should be retried, false otherwise
     */
    suspend fun onAuthError(errorMessage: String): Boolean

    companion object {
        /**
         * Default no-op handler that never retries.
         */
        val NoOp = AuthErrorHandler { false }
    }
}
