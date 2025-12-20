package com.lelloman.pezzottify.android.domain.auth.oidc

/**
 * Configuration for OIDC authentication.
 */
data class OidcConfig(
    /** The OIDC issuer URL (e.g., https://auth.lelloman.com) */
    val issuerUrl: String,
    /** The client ID registered with the OIDC provider */
    val clientId: String,
    /** The redirect URI for receiving the authorization callback */
    val redirectUri: String,
    /** The scopes to request */
    val scopes: List<String> = listOf("openid", "profile", "email"),
)
