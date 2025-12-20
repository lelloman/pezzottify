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
import net.openid.appauth.GrantTypeValues
import net.openid.appauth.ResponseTypeValues
import net.openid.appauth.TokenRequest
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
    private val prefs by lazy {
        logger.debug("[OIDC_DBG] prefs lazy init, manager instance=${this.hashCode()}")
        context.getSharedPreferences("oidc_auth", Context.MODE_PRIVATE).also {
            logger.debug("[OIDC_DBG] prefs initialized, all keys=${it.all.keys}")
        }
    }

    init {
        logger.debug("[OIDC_DBG] AppAuthOidcManager created, instance=${this.hashCode()}, context=${context.hashCode()}")
    }

    private var serviceConfig: AuthorizationServiceConfiguration? = null

    private var pendingAuthRequest: AuthorizationRequest?
        get() {
            val json = prefs.getString(KEY_PENDING_REQUEST, null)
            logger.debug("[OIDC_DBG] get() raw json length=${json?.length}, contains=${prefs.contains(KEY_PENDING_REQUEST)}")
            if (json == null) {
                logger.debug("[OIDC_DBG] get() no saved request")
                return null
            }
            return try {
                AuthorizationRequest.jsonDeserialize(json).also {
                    logger.debug("[OIDC_DBG] get() recovered request, state=${it.state}")
                }
            } catch (e: Exception) {
                logger.error("[OIDC_DBG] get() failed to deserialize: ${e.message}", e)
                null
            }
        }
        set(value) {
            if (value != null) {
                val json = value.jsonSerializeString()
                logger.debug("[OIDC_DBG] set() about to save, json length=${json.length}, state=${value.state}")
                val success = prefs.edit().putString(KEY_PENDING_REQUEST, json).commit()
                logger.debug("[OIDC_DBG] set() commit result=$success")
                // Verify immediately
                val verification = prefs.getString(KEY_PENDING_REQUEST, null)
                logger.debug("[OIDC_DBG] set() verification: saved=${verification != null}, length=${verification?.length}")
            } else {
                prefs.edit().remove(KEY_PENDING_REQUEST).commit()
                logger.debug("[OIDC_DBG] set() cleared request")
            }
        }

    override suspend fun createAuthorizationIntent(deviceInfo: DeviceInfo): Intent? {
        logger.debug("[OIDC_DBG] createAuthorizationIntent() starting")

        val config = getServiceConfiguration()
        if (config == null) {
            logger.error("[OIDC_DBG] createAuthorizationIntent() config is null, aborting")
            return null
        }

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

        authRequestBuilder
            .setPrompt("login")
            .setAdditionalParameters(additionalParams)

        val authRequest = authRequestBuilder.build()
        logger.debug("[OIDC_DBG] createAuthorizationIntent() built request, state=${authRequest.state}, saving...")
        pendingAuthRequest = authRequest

        logger.debug("[OIDC_DBG] createAuthorizationIntent() done, device_id=${deviceInfo.deviceUuid}")

        return authService.getAuthorizationRequestIntent(authRequest)
    }

    override suspend fun handleAuthorizationResponse(intent: Intent): OidcAuthManager.AuthorizationResult {
        logger.debug("[OIDC_DBG] handleAuthorizationResponse() called")
        // First try to get response from AppAuth-formatted intent (has extras)
        var response = AuthorizationResponse.fromIntent(intent)
        var exception = AuthorizationException.fromIntent(intent)

        // If no response/exception from extras, try to build from URI (deep link callback)
        if (response == null && exception == null) {
            val uri = intent.data
            val request = pendingAuthRequest

            if (uri != null) {
                val hasCode = uri.getQueryParameter("code") != null
                val hasError = uri.getQueryParameter("error") != null
                logger.debug("[OIDC_DBG] handleResponse() URI hasCode=$hasCode, hasError=$hasError, hasRequest=${request != null}")

                when {
                    hasError -> {
                        // Parse error from URI
                        exception = AuthorizationException.fromOAuthRedirect(uri)
                        logger.debug("[OIDC_DBG] handleResponse() parsed error: ${exception?.code}")
                    }
                    hasCode && request != null -> {
                        // Build success response from URI
                        response = try {
                            AuthorizationResponse.Builder(request)
                                .fromUri(uri)
                                .build()
                                .also { logger.debug("[OIDC_DBG] handleResponse() built response, code=${it.authorizationCode?.take(10)}...") }
                        } catch (e: Exception) {
                            logger.error("[OIDC_DBG] handleResponse() failed to parse URI: ${e.message}")
                            null
                        }
                    }
                    hasCode && request == null -> {
                        logger.warn("[OIDC_DBG] handleResponse() has code but no pending request!")
                        return OidcAuthManager.AuthorizationResult.Error("No pending auth request - please try again")
                    }
                    else -> {
                        logger.warn("[OIDC_DBG] handleResponse() URI has neither code nor error")
                        return OidcAuthManager.AuthorizationResult.Error("Invalid callback - no code or error in response")
                    }
                }
            } else {
                logger.warn("[OIDC_DBG] handleResponse() no URI in intent")
                return OidcAuthManager.AuthorizationResult.Error("No callback URI in intent")
            }
        }

        return when {
            exception != null -> {
                // Clear request on explicit error/cancel
                pendingAuthRequest = null
                if (exception.code == AuthorizationException.GeneralErrors.USER_CANCELED_AUTH_FLOW.code) {
                    logger.info("[OIDC_DBG] handleResponse() user cancelled")
                    OidcAuthManager.AuthorizationResult.Cancelled
                } else {
                    logger.error("[OIDC_DBG] handleResponse() error: ${exception.errorDescription}")
                    OidcAuthManager.AuthorizationResult.Error(
                        exception.errorDescription ?: "Authorization failed"
                    )
                }
            }

            response != null -> {
                // Clear request on success
                pendingAuthRequest = null
                logger.debug("[OIDC_DBG] handleResponse() got code, exchanging for tokens")
                exchangeCodeForTokens(response)
            }

            else -> {
                // This shouldn't happen - we handle all cases above
                logger.error("[OIDC_DBG] handleResponse() unexpected state: no response or exception")
                OidcAuthManager.AuthorizationResult.Error("Unexpected authorization state")
            }
        }
    }

    private suspend fun exchangeCodeForTokens(
        response: AuthorizationResponse
    ): OidcAuthManager.AuthorizationResult = withContext(Dispatchers.IO) {
        logger.debug("[OIDC_DBG] exchangeCodeForTokens() starting")
        suspendCancellableCoroutine { continuation ->
            authService.performTokenRequest(response.createTokenExchangeRequest()) { tokenResponse, exception ->
                logger.debug("[OIDC_DBG] exchangeCodeForTokens() callback received")
                when {
                    exception != null -> {
                        logger.error("[OIDC_DBG] exchangeCodeForTokens() error: ${exception.errorDescription}", exception)
                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Error(
                                exception.errorDescription ?: "Token exchange failed"
                            )
                        )
                    }

                    tokenResponse != null -> {
                        logger.debug("[OIDC_DBG] exchangeCodeForTokens() got token response")
                        val idToken = tokenResponse.idToken
                        if (idToken == null) {
                            logger.error("[OIDC_DBG] exchangeCodeForTokens() no ID token in response")
                            continuation.resume(
                                OidcAuthManager.AuthorizationResult.Error("No ID token received")
                            )
                            return@performTokenRequest
                        }

                        // Extract user info from ID token
                        val userHandle = extractUserHandle(idToken)
                        val refreshToken = tokenResponse.refreshToken
                        logger.info("[OIDC_DBG] exchangeCodeForTokens() success, user: $userHandle, hasRefreshToken: ${refreshToken != null}")

                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Success(
                                idToken = idToken,
                                refreshToken = refreshToken,
                                userHandle = userHandle,
                            )
                        )
                    }

                    else -> {
                        logger.error("[OIDC_DBG] exchangeCodeForTokens() no response")
                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Error("No token response")
                        )
                    }
                }
            }
        }
    }

    override suspend fun refreshTokens(refreshToken: String): OidcAuthManager.RefreshResult =
        withContext(Dispatchers.IO) {
            logger.debug("[OIDC_DBG] refreshTokens() starting")

            val config = getServiceConfiguration()
            if (config == null) {
                logger.error("[OIDC_DBG] refreshTokens() config is null")
                return@withContext OidcAuthManager.RefreshResult.Failed("OIDC not configured")
            }

            val tokenRequest = TokenRequest.Builder(config, oidcConfig.clientId)
                .setGrantType(GrantTypeValues.REFRESH_TOKEN)
                .setRefreshToken(refreshToken)
                .setScopes(oidcConfig.scopes.toSet())
                .build()

            suspendCancellableCoroutine<OidcAuthManager.RefreshResult> { continuation ->
                authService.performTokenRequest(tokenRequest) { tokenResponse, exception ->
                    logger.debug("[OIDC_DBG] refreshTokens() callback received")
                    val result: OidcAuthManager.RefreshResult = when {
                        exception != null -> {
                            logger.error("[OIDC_DBG] refreshTokens() error: ${exception.errorDescription}", exception)
                            OidcAuthManager.RefreshResult.Failed(
                                exception.errorDescription ?: "Token refresh failed"
                            )
                        }

                        tokenResponse != null -> {
                            val idToken = tokenResponse.idToken
                            if (idToken == null) {
                                logger.error("[OIDC_DBG] refreshTokens() no ID token in response")
                                OidcAuthManager.RefreshResult.Failed("No ID token received")
                            } else {
                                val newRefreshToken = tokenResponse.refreshToken
                                logger.info("[OIDC_DBG] refreshTokens() success, hasNewRefreshToken: ${newRefreshToken != null}")
                                OidcAuthManager.RefreshResult.Success(
                                    idToken = idToken,
                                    refreshToken = newRefreshToken ?: refreshToken,
                                )
                            }
                        }

                        else -> {
                            logger.error("[OIDC_DBG] refreshTokens() no response")
                            OidcAuthManager.RefreshResult.Failed("No token response")
                        }
                    }
                    continuation.resume(result)
                }
            }
        }

    private suspend fun getServiceConfiguration(): AuthorizationServiceConfiguration? {
        serviceConfig?.let {
            logger.debug("[OIDC_DBG] getServiceConfig() using cached")
            return it
        }

        logger.debug("[OIDC_DBG] getServiceConfig() fetching from: ${oidcConfig.issuerUrl}")
        return withContext(Dispatchers.IO) {
            suspendCancellableCoroutine { continuation ->
                AuthorizationServiceConfiguration.fetchFromIssuer(
                    Uri.parse(oidcConfig.issuerUrl)
                ) { config, exception ->
                    if (exception != null) {
                        logger.error("[OIDC_DBG] getServiceConfig() failed: ${exception.message}", exception)
                        continuation.resume(null)
                    } else {
                        logger.debug("[OIDC_DBG] getServiceConfig() success: auth=${config?.authorizationEndpoint}")
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
