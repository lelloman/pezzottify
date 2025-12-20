package com.lelloman.pezzottify.android.oidc

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcConfig
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import net.openid.appauth.AuthorizationException
import net.openid.appauth.AuthorizationRequest
import net.openid.appauth.AuthorizationResponse
import net.openid.appauth.AuthorizationService
import net.openid.appauth.AuthorizationServiceConfiguration
import net.openid.appauth.ResponseTypeValues
import org.json.JSONObject
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.resume

@Singleton
class AppAuthOidcManager @Inject constructor(
    @ApplicationContext private val context: Context,
    private val oidcConfig: OidcConfig,
    loggerFactory: LoggerFactory,
) : OidcAuthManager {

    private val logger: Logger by loggerFactory
    private val authService: AuthorizationService by lazy { AuthorizationService(context) }
    private val prefs by lazy { context.getSharedPreferences("oidc_auth", Context.MODE_PRIVATE) }

    private var serviceConfig: AuthorizationServiceConfiguration? = null

    private var pendingAuthRequest: AuthorizationRequest?
        get() = prefs.getString(KEY_PENDING_REQUEST, null)?.let {
            try { AuthorizationRequest.jsonDeserialize(it) } catch (e: Exception) { null }
        }
        set(value) {
            if (value != null) {
                prefs.edit().putString(KEY_PENDING_REQUEST, value.jsonSerializeString()).apply()
            } else {
                prefs.edit().remove(KEY_PENDING_REQUEST).apply()
            }
        }

    override suspend fun createAuthorizationIntent(deviceInfo: DeviceInfo): Intent? {
        logger.debug("createAuthorizationIntent() starting OIDC flow")

        val config = getServiceConfiguration() ?: return null

        // Build the authorization request with device info as additional parameters
        val authRequestBuilder = AuthorizationRequest.Builder(
            config,
            oidcConfig.clientId,
            ResponseTypeValues.CODE,
            Uri.parse(oidcConfig.redirectUri)
        ).setScopes(oidcConfig.scopes)

        // Add device info as additional parameters
        val additionalParams = mutableMapOf<String, String>()
        additionalParams["device_id"] = deviceInfo.deviceUuid
        additionalParams["device_type"] = deviceInfo.deviceType
        deviceInfo.deviceName?.let { additionalParams["device_name"] = it }

        authRequestBuilder.setAdditionalParameters(additionalParams)

        val authRequest = authRequestBuilder.build()
        pendingAuthRequest = authRequest

        logger.debug("createAuthorizationIntent() built auth request with device_id=${deviceInfo.deviceUuid}, state=${authRequest.state}")

        return authService.getAuthorizationRequestIntent(authRequest)
    }

    override suspend fun handleAuthorizationResponse(intent: Intent): OidcAuthManager.AuthorizationResult {
        // First try to get response from AppAuth-formatted intent (has extras)
        var response = AuthorizationResponse.fromIntent(intent)
        var exception = AuthorizationException.fromIntent(intent)

        // If no response/exception from extras, try to build from URI (deep link callback)
        if (response == null && exception == null) {
            val uri = intent.data
            val request = pendingAuthRequest

            if (uri != null) {
                logger.debug("handleAuthorizationResponse() URI: $uri, hasRequest=${request != null}")

                // Check for error in URI first (works even without request)
                exception = AuthorizationException.fromOAuthRedirect(uri)
                logger.debug("handleAuthorizationResponse() fromOAuthRedirect returned: ${exception?.code}")

                if (exception == null && request != null) {
                    // Build response from URI (requires the original request)
                    response = try {
                        AuthorizationResponse.Builder(request)
                            .fromUri(uri)
                            .build()
                            .also { logger.debug("handleAuthorizationResponse() built response, code=${it.authorizationCode}") }
                    } catch (e: Exception) {
                        logger.error("handleAuthorizationResponse() failed to parse URI: ${e.message}")
                        null
                    }
                } else if (exception == null && request == null) {
                    logger.warn("handleAuthorizationResponse() got success URI but no pending request (process was killed?)")
                }
            } else {
                logger.warn("handleAuthorizationResponse() no URI in intent")
            }
        }

        // Clear pending request after handling
        pendingAuthRequest = null

        return when {
            exception != null -> {
                if (exception.code == AuthorizationException.GeneralErrors.USER_CANCELED_AUTH_FLOW.code) {
                    logger.info("handleAuthorizationResponse() user cancelled")
                    OidcAuthManager.AuthorizationResult.Cancelled
                } else {
                    logger.error("handleAuthorizationResponse() error: ${exception.errorDescription}")
                    OidcAuthManager.AuthorizationResult.Error(
                        exception.errorDescription ?: "Authorization failed"
                    )
                }
            }

            response != null -> {
                logger.debug("handleAuthorizationResponse() got authorization code, exchanging for tokens")
                exchangeCodeForTokens(response)
            }

            else -> {
                logger.error("handleAuthorizationResponse() no response or exception")
                OidcAuthManager.AuthorizationResult.Error("No authorization response")
            }
        }
    }

    private suspend fun exchangeCodeForTokens(
        response: AuthorizationResponse
    ): OidcAuthManager.AuthorizationResult = withContext(Dispatchers.IO) {
        suspendCancellableCoroutine { continuation ->
            authService.performTokenRequest(response.createTokenExchangeRequest()) { tokenResponse, exception ->
                when {
                    exception != null -> {
                        logger.error("exchangeCodeForTokens() error: ${exception.errorDescription}")
                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Error(
                                exception.errorDescription ?: "Token exchange failed"
                            )
                        )
                    }

                    tokenResponse != null -> {
                        val idToken = tokenResponse.idToken
                        if (idToken == null) {
                            logger.error("exchangeCodeForTokens() no ID token in response")
                            continuation.resume(
                                OidcAuthManager.AuthorizationResult.Error("No ID token received")
                            )
                            return@performTokenRequest
                        }

                        // Extract user info from ID token
                        val userHandle = extractUserHandle(idToken)
                        logger.info("exchangeCodeForTokens() success, user: $userHandle")

                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Success(
                                idToken = idToken,
                                userHandle = userHandle,
                            )
                        )
                    }

                    else -> {
                        logger.error("exchangeCodeForTokens() no response")
                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Error("No token response")
                        )
                    }
                }
            }
        }
    }

    private suspend fun getServiceConfiguration(): AuthorizationServiceConfiguration? {
        serviceConfig?.let { return it }

        return withContext(Dispatchers.IO) {
            suspendCancellableCoroutine { continuation ->
                AuthorizationServiceConfiguration.fetchFromIssuer(
                    Uri.parse(oidcConfig.issuerUrl)
                ) { config, exception ->
                    if (exception != null) {
                        logger.error("getServiceConfiguration() failed: ${exception.message}")
                        continuation.resume(null)
                    } else {
                        logger.debug("getServiceConfiguration() discovered OIDC endpoints")
                        serviceConfig = config
                        continuation.resume(config)
                    }
                }
            }
        }
    }

    /**
     * Extract user handle from ID token JWT.
     * Tries preferred_username, then email, then subject.
     */
    private fun extractUserHandle(idToken: String): String {
        return try {
            val parts = idToken.split(".")
            if (parts.size != 3) return "user"

            val payload = String(
                android.util.Base64.decode(parts[1], android.util.Base64.URL_SAFE)
            )
            val json = JSONObject(payload)

            json.optString("preferred_username").takeIf { it.isNotBlank() }
                ?: json.optString("email").takeIf { it.isNotBlank() }
                ?: json.optString("sub").takeIf { it.isNotBlank() }
                ?: "user"
        } catch (e: Exception) {
            logger.warn("extractUserHandle() failed to parse ID token: ${e.message}")
            "user"
        }
    }

    companion object {
        private const val KEY_PENDING_REQUEST = "pending_auth_request"
    }
}
