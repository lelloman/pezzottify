package com.lelloman.pezzottify.android.domain.auth

/**
 * Handler for session expiration events.
 * Called when the server returns 401/403, indicating the auth token
 * is invalid, expired, or revoked.
 */
interface SessionExpiredHandler {
    /**
     * Called when a session expiration is detected.
     * Implementations should trigger logout and navigate to the login screen.
     */
    fun onSessionExpired()
}
