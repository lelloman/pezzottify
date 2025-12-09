package com.lelloman.pezzottify.android.remoteapi.internal

import android.util.Base64
import com.lelloman.pezzottify.android.domain.config.SslPinConfig
import okhttp3.OkHttpClient
import java.net.URI
import java.security.MessageDigest
import java.security.cert.X509Certificate
import javax.inject.Inject
import javax.inject.Singleton
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

/**
 * Factory for creating OkHttpClient.Builder instances with optional SSL certificate pinning.
 *
 * When SSL pinning is enabled (via build-time configuration), all HTTPS connections
 * will trust certificates that match the pinned public key hash. This allows using
 * self-signed certificates while maintaining security through pinning.
 */
@Singleton
class OkHttpClientFactory @Inject constructor(
    private val sslPinConfig: SslPinConfig,
) {
    /**
     * Creates a new OkHttpClient.Builder with certificate pinning configured if enabled.
     *
     * When a pin hash is configured, the builder will trust certificates whose public key
     * matches the pinned hash, even if they are self-signed.
     *
     * @param baseUrl The base URL (used for logging/debugging, pinning applies to all HTTPS)
     * @return OkHttpClient.Builder configured with certificate pinning if enabled
     */
    fun createBuilder(baseUrl: String): OkHttpClient.Builder {
        val builder = OkHttpClient.Builder()

        if (sslPinConfig.isEnabled && baseUrl.startsWith("https://")) {
            val trustManager = PinningTrustManager(sslPinConfig.pinHash)
            val sslContext = SSLContext.getInstance("TLS")
            sslContext.init(null, arrayOf<TrustManager>(trustManager), null)

            builder.sslSocketFactory(sslContext.socketFactory, trustManager)
            builder.hostnameVerifier { _, _ -> true } // Host verification done via pin
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

/**
 * X509TrustManager that trusts certificates matching the pinned public key hash.
 *
 * This allows self-signed certificates to be trusted while maintaining security -
 * only certificates with the exact public key we expect will be accepted.
 */
private class PinningTrustManager(
    private val expectedPinHash: String,
) : X509TrustManager {

    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        // Client certificate validation not needed for our use case
    }

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        if (chain.isNullOrEmpty()) {
            throw java.security.cert.CertificateException("Empty certificate chain")
        }

        val serverCert = chain[0]
        val publicKeyHash = computePublicKeyHash(serverCert)
        val expectedHash = expectedPinHash.removePrefix("sha256/")

        if (publicKeyHash != expectedHash) {
            throw java.security.cert.CertificateException(
                "Certificate pin mismatch. Expected: $expectedHash, Got: $publicKeyHash"
            )
        }
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()

    private fun computePublicKeyHash(certificate: X509Certificate): String {
        val publicKeyBytes = certificate.publicKey.encoded
        val digest = MessageDigest.getInstance("SHA-256")
        val hash = digest.digest(publicKeyBytes)
        return Base64.encodeToString(hash, Base64.NO_WRAP)
    }
}
