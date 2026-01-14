package com.lelloman.pezzottify.android.domain.auth.oidc

import android.content.Intent
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo

/**
 * Manages OIDC authentication flow.
 *
 * The flow is:
 * 1. Call [createAuthorizationIntent] to get an Intent to launch the browser
 * 2. Launch the intent with startActivityForResult
 * 3. When the browser redirects back, pass the result intent to [handleAuthorizationResponse]
 */
interface OidcAuthManager {

    /**
     * Creates an Intent to launch the authorization flow.
     *
     * @param deviceInfo Device information to pass to the OIDC provider
     * @param loginHint Optional username hint to pre-fill on the login page
     * @return Intent to launch, or null if OIDC is not configured
     */
    suspend fun createAuthorizationIntent(deviceInfo: DeviceInfo, loginHint: String? = null): Intent?

    /**
     * Handles the authorization response from the browser redirect.
     *
     * @param intent The intent received from the OIDC callback
     * @return The result of the authorization
     */
    suspend fun handleAuthorizationResponse(intent: Intent): AuthorizationResult

    /**
     * Refreshes the tokens using a refresh token.
     *
     * @param refreshToken The refresh token to use
     * @return The result of the refresh operation
     */
    suspend fun refreshTokens(refreshToken: String): RefreshResult

    /**
     * Result of the OIDC authorization flow.
     */
    sealed interface AuthorizationResult {
        /**
         * Authorization successful.
         * @param idToken The ID token (JWT) to use as session token
         * @param refreshToken The refresh token for obtaining new tokens
         * @param userHandle The user's identifier (email or preferred_username)
         */
        data class Success(
            val idToken: String,
            val refreshToken: String?,
            val userHandle: String,
        ) : AuthorizationResult

        /**
         * User cancelled the authorization.
         */
        data object Cancelled : AuthorizationResult

        /**
         * Authorization failed with an error.
         */
        data class Error(val message: String) : AuthorizationResult
    }

    /**
     * Result of a token refresh operation.
     */
    sealed interface RefreshResult {
        /**
         * Token refresh successful.
         * @param idToken The new ID token (JWT), or null if provider didn't return one
         * @param refreshToken The new refresh token (may be same as before or rotated)
         */
        data class Success(
            val idToken: String?,
            val refreshToken: String?,
        ) : RefreshResult

        /**
         * Token refresh failed - user needs to re-authenticate.
         */
        data class Failed(val message: String) : RefreshResult

        /**
         * Token refresh was rate limited by the OIDC provider.
         * @param retryAfterMs Suggested delay before retrying, in milliseconds.
         */
        data class RateLimited(val retryAfterMs: Long) : RefreshResult
    }
}
