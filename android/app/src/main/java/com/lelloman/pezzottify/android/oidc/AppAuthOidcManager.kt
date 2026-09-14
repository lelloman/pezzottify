package com.lelloman.pezzottify.android.oidc

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.lelloman.pezzottify.android.BuildConfig
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
    private val prefs by lazy { context.getSharedPreferences("oidc_auth", Context.MODE_PRIVATE) }
    private var serviceConfig: AuthorizationServiceConfiguration? = null
    private var pendingAuthRequest: AuthorizationRequest?
        get() {
            val created = prefs.getLong("pending_created_at", 0)
            if (System.currentTimeMillis() - created !in 0..600_000) return null
            return prefs.getString(KEY_PENDING_REQUEST, null)?.let { runCatching { AuthorizationRequest.jsonDeserialize(it) }.getOrNull() }
        }
        set(value) {
            val edit = prefs.edit()
            if (value == null) edit.remove(KEY_PENDING_REQUEST).remove("pending_created_at")
            else edit.putString(KEY_PENDING_REQUEST, value.jsonSerializeString()).putLong("pending_created_at", System.currentTimeMillis())
            check(edit.commit())
        }

    override suspend fun createAuthorizationIntent(deviceInfo: DeviceInfo, loginHint: String?): Intent? {

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

        // Add login hint if provided (pre-fills username on login page)
        if (!loginHint.isNullOrBlank()) {
            authRequestBuilder.setLoginHint(loginHint)
        }

        val authRequest = authRequestBuilder.build()
        pendingAuthRequest = authRequest


        return AuthenticatorLauncher(context, BuildConfig.AUTHENTICATOR_ENABLED && !BuildConfig.IS_TV,
            BuildConfig.AUTHENTICATOR_PACKAGE, BuildConfig.AUTHENTICATOR_CERTIFICATE).launch(authRequest, oidcConfig.issuerUrl)
            ?: authService.getAuthorizationRequestIntent(authRequest)
    }

    override suspend fun handleAuthorizationResponse(intent: Intent): OidcAuthManager.AuthorizationResult {
        val request = pendingAuthRequest ?: return OidcAuthManager.AuthorizationResult.Error("No pending sign-in. Please try again.")
        var response: AuthorizationResponse?
        var exception: AuthorizationException?
        try {
            response = AuthorizationResponse.fromIntent(intent)
            exception = AuthorizationException.fromIntent(intent)
            if (response == null && exception == null) {
                val uri = checkNotNull(intent.data)
                AuthorizationCallbacks.validate(uri, request)
                if (uri.getQueryParameter("error") != null) exception = AuthorizationException.fromOAuthRedirect(uri)
                else response = AuthorizationResponse.Builder(request).fromUri(uri).build()
            }
            response?.let { AuthorizationCallbacks.validate(it, request) }
            require(response == null || exception == null)
        } catch (_: Exception) {
            // An unrelated or tampered callback must not consume the legitimate pending request.
            return OidcAuthManager.AuthorizationResult.Error("Invalid sign-in callback")
        }
        if (exception != null) {
            pendingAuthRequest = null
            return if (exception.code == AuthorizationException.GeneralErrors.USER_CANCELED_AUTH_FLOW.code || exception.error == "access_denied")
                OidcAuthManager.AuthorizationResult.Cancelled
            else OidcAuthManager.AuthorizationResult.Error("Authorization failed")
        }
        val approved = response ?: return OidcAuthManager.AuthorizationResult.Error("Invalid sign-in callback")
        pendingAuthRequest = null
        return exchangeCodeForTokens(approved)
    }

    private suspend fun exchangeCodeForTokens(
        response: AuthorizationResponse
    ): OidcAuthManager.AuthorizationResult = withContext(Dispatchers.IO) {
        suspendCancellableCoroutine { continuation ->
            authService.performTokenRequest(response.createTokenExchangeRequest()) { tokenResponse, exception ->
                when {
                    exception != null -> {
                        logger.error("OIDC operation failed")
                        continuation.resume(
                            OidcAuthManager.AuthorizationResult.Error(
                                exception.errorDescription ?: "Token exchange failed"
                            )
                        )
                    }

                    tokenResponse != null -> {
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
                        logger.info("OIDC operation completed")

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
                    val result: OidcAuthManager.RefreshResult = when {
                        exception != null -> {
                            logger.error("OIDC operation failed")

                            // Check for rate limiting (HTTP 429)
                            // AppAuth returns SERVER_ERROR for HTTP errors, check the description
                            val isRateLimited = exception.type == AuthorizationException.TYPE_OAUTH_TOKEN_ERROR &&
                                (exception.errorDescription?.contains("429", ignoreCase = true) == true ||
                                    exception.errorDescription?.contains("too many", ignoreCase = true) == true ||
                                    exception.errorDescription?.contains("rate limit", ignoreCase = true) == true)

                            if (isRateLimited) {
                                logger.warn("[OIDC_DBG] refreshTokens() rate limited by OIDC provider")
                                OidcAuthManager.RefreshResult.RateLimited(retryAfterMs = DEFAULT_RATE_LIMIT_BACKOFF_MS)
                            } else {
                                OidcAuthManager.RefreshResult.Failed(
                                    exception.errorDescription ?: "Token refresh failed"
                                )
                            }
                        }

                        tokenResponse != null -> {
                            // Return ID token if available, null otherwise
                            // The caller should keep the old ID token if we don't get a new one
                            val newRefreshToken = tokenResponse.refreshToken
                            logger.info("OIDC operation completed")
                            OidcAuthManager.RefreshResult.Success(
                                idToken = tokenResponse.idToken,
                                refreshToken = newRefreshToken ?: refreshToken,
                            )
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
            return it
        }

        return withContext(Dispatchers.IO) {
            suspendCancellableCoroutine { continuation ->
                AuthorizationServiceConfiguration.fetchFromIssuer(
                    Uri.parse(oidcConfig.issuerUrl)
                ) { config, exception ->
                    if (exception != null) {
                        logger.error("OIDC operation failed")
                        continuation.resume(null)
                    } else {
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
            logger.warn("OIDC operation failed")
            "user"
        }
    }

    companion object {
        private const val KEY_PENDING_REQUEST = "pending_auth_request"
        private const val DEFAULT_RATE_LIMIT_BACKOFF_MS = 60_000L // 1 minute default backoff
    }
}
