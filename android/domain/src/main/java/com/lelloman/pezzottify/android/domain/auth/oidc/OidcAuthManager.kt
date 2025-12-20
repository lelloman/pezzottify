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
     * @return Intent to launch, or null if OIDC is not configured
     */
    suspend fun createAuthorizationIntent(deviceInfo: DeviceInfo): Intent?

    /**
     * Handles the authorization response from the browser redirect.
     *
     * @param intent The intent received from the OIDC callback
     * @return The result of the authorization
     */
    suspend fun handleAuthorizationResponse(intent: Intent): AuthorizationResult

    /**
     * Result of the OIDC authorization flow.
     */
    sealed interface AuthorizationResult {
        /**
         * Authorization successful.
         * @param idToken The ID token (JWT) to use as session token
         * @param userHandle The user's identifier (email or preferred_username)
         */
        data class Success(
            val idToken: String,
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
}
