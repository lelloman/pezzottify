package com.lelloman.pezzottify.android.domain.config

/**
 * SSL certificate pinning configuration.
 * Configured at build time, not modifiable at runtime.
 */
interface SslPinConfig {
    /**
     * The SHA-256 hash of the certificate's public key.
     * Format: "sha256/BASE64_ENCODED_HASH"
     * Empty string means pinning is disabled.
     */
    val pinHash: String

    /**
     * Whether SSL certificate pinning is enabled.
     */
    val isEnabled: Boolean
        get() = pinHash.isNotBlank()
}
