package com.lelloman.pezzottify.android.domain.auth.oidc

import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo

/**
 * OAuth 2.0 Device Authorization Grant (RFC 8628) against the configured OIDC issuer.
 *
 * Used on devices without a comfortable browser or keyboard (Android TV): the device shows a
 * short code, the user approves it on a phone, and the device polls the token endpoint until
 * the approval arrives. The resulting ID token is the same one the browser flow produces for
 * the same client id, so it is stored and refreshed exactly like a browser sign-in.
 */
interface DeviceFlowAuthManager {

    /**
     * Requests a new device code and user code from the issuer.
     *
     * @param deviceInfo Sent like on the browser flow's authorization request, so the issuer
     * puts the device claims the Pezzottify server needs into the ID token.
     */
    suspend fun start(deviceInfo: DeviceInfo): StartResult

    /**
     * Asks the token endpoint once whether [deviceCode] has been approved.
     * Callers are responsible for waiting [DeviceAuthorization.intervalMs] between calls.
     */
    suspend fun poll(deviceCode: String): PollResult

    data class DeviceAuthorization(
        val deviceCode: String,
        val userCode: String,
        val verificationUri: String,
        val verificationUriComplete: String?,
        /** Optional LelloAuth extension, preferred over [verificationUriComplete] for the QR code. */
        val qrCodeUrl: String?,
        /** Lifetime of the device code, relative to when it was received. */
        val expiresInMs: Long,
        /** Minimum delay between two polls. */
        val intervalMs: Long,
    )

    sealed interface StartResult {
        data class Success(val authorization: DeviceAuthorization) : StartResult
        data class Error(val message: String) : StartResult
    }

    sealed interface PollResult {
        /** `authorization_pending`: keep polling at the current interval. */
        data object Pending : PollResult

        /** `slow_down`: keep polling, with the interval raised by 5 seconds. */
        data object SlowDown : PollResult

        data class Approved(
            val idToken: String,
            val refreshToken: String?,
            val userHandle: String,
        ) : PollResult

        /** `access_denied`: the user rejected the request. */
        data object Denied : PollResult

        /** `expired_token`: the device code is no longer valid; a new one is needed. */
        data object Expired : PollResult

        /** Network, server or protocol failure; polling may be retried. */
        data class Error(val message: String) : PollResult
    }

    companion object {
        /** RFC 8628 §3.5: each `slow_down` raises the polling interval by 5 seconds. */
        const val SLOW_DOWN_INCREMENT_MS = 5_000L

        /** RFC 8628 §3.2: interval to use when the server does not send one. */
        const val DEFAULT_INTERVAL_MS = 5_000L

        const val DEVICE_CODE_GRANT_TYPE = "urn:ietf:params:oauth:grant-type:device_code"
    }
}
