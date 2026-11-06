package com.lelloman.pezzottify.android.oidc

/** Endpoints the device flow needs, taken from the issuer's discovery document. */
interface DeviceFlowEndpointsProvider {

    /** Null when discovery failed (e.g. no network); callers may retry later. */
    suspend fun deviceFlowEndpoints(): Endpoints?

    data class Endpoints(
        /** Null when the issuer does not advertise `device_authorization_endpoint`. */
        val deviceAuthorizationEndpoint: String?,
        val tokenEndpoint: String,
    )
}
