package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.config.SslPinConfig
import okhttp3.CertificatePinner
import okhttp3.OkHttpClient
import java.net.URI
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Factory for creating OkHttpClient.Builder instances with optional SSL certificate pinning.
 *
 * When SSL pinning is enabled (via build-time configuration), all HTTPS connections
 * to the specified host will verify the server's certificate against the pinned hash.
 */
@Singleton
class OkHttpClientFactory @Inject constructor(
    private val sslPinConfig: SslPinConfig,
) {
    /**
     * Creates a new OkHttpClient.Builder with certificate pinning configured if enabled.
     *
     * @param baseUrl The base URL to extract the host from for pinning
     * @return OkHttpClient.Builder configured with certificate pinning if enabled
     */
    fun createBuilder(baseUrl: String): OkHttpClient.Builder {
        val builder = OkHttpClient.Builder()

        if (sslPinConfig.isEnabled && baseUrl.startsWith("https://")) {
            val host = extractHost(baseUrl)
            if (host != null) {
                val certificatePinner = CertificatePinner.Builder()
                    .add(host, sslPinConfig.pinHash)
                    .build()
                builder.certificatePinner(certificatePinner)
            }
        }

        return builder
    }

    private fun extractHost(url: String): String? {
        return try {
            URI(url).host
        } catch (_: Exception) {
            null
        }
    }
}
