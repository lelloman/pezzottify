package com.lelloman.pezzottify.android.oidc

import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.Companion.DEFAULT_INTERVAL_MS
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.Companion.DEVICE_CODE_GRANT_TYPE
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.PollResult
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.StartResult
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcConfig
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import okhttp3.Call
import okhttp3.Callback
import okhttp3.FormBody
import okhttp3.Request
import okhttp3.Response
import java.io.IOException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * RFC 8628 device authorization grant over plain form posts.
 *
 * AppAuth has no device-grant support, so this talks to the endpoints directly; discovery is
 * shared with [AppAuthOidcManager] through [DeviceFlowEndpointsProvider]. Each [poll] is a single
 * request: interval, `slow_down` and expiry are handled by the caller's polling loop.
 */
class OkHttpDeviceFlowAuthManager(
    private val httpClient: Call.Factory,
    private val endpointsProvider: DeviceFlowEndpointsProvider,
    private val oidcConfig: OidcConfig,
    loggerFactory: LoggerFactory,
) : DeviceFlowAuthManager {

    private val logger: Logger by loggerFactory

    override suspend fun start(deviceInfo: DeviceInfo): StartResult {
        val endpoints = endpointsProvider.deviceFlowEndpoints()
            ?: return StartResult.Error("Cannot reach the sign-in server")
        val deviceEndpoint = endpoints.deviceAuthorizationEndpoint
            ?: return StartResult.Error("The sign-in server does not support signing in from another device")

        // Same device parameters as the browser flow's authorization request (AppAuthOidcManager).
        val form = FormBody.Builder()
            .add("client_id", oidcConfig.clientId)
            .add("scope", oidcConfig.scopes.joinToString(" "))
            .add("device_id", deviceInfo.deviceUuid)
            .add("device_type", deviceInfo.deviceType)
            .apply { deviceInfo.deviceName?.let { add("device_name", it) } }
            .build()

        val (code, json) = try {
            post(deviceEndpoint, form)
        } catch (e: IOException) {
            logger.warn("start() device authorization request failed: ${e.javaClass.simpleName}")
            return StartResult.Error("Cannot reach the sign-in server")
        }

        if (code !in 200..299 || json == null) {
            val error = json?.string("error_description") ?: json?.string("error") ?: "HTTP $code"
            logger.warn("start() device authorization rejected: $error")
            return StartResult.Error(error)
        }

        val deviceCode = json.string("device_code")
        val userCode = json.string("user_code")
        val verificationUri = json.string("verification_uri")
        if (deviceCode == null || userCode == null || verificationUri == null) {
            logger.error("start() malformed device authorization response")
            return StartResult.Error("Unexpected response from the sign-in server")
        }
        val expiresInS = json.long("expires_in")?.takeIf { it > 0 } ?: DEFAULT_EXPIRES_IN_S
        val intervalS = json.long("interval")?.takeIf { it > 0 }
        logger.info("start() device code issued, expires in ${expiresInS}s")
        return StartResult.Success(
            DeviceFlowAuthManager.DeviceAuthorization(
                deviceCode = deviceCode,
                userCode = userCode,
                verificationUri = verificationUri,
                verificationUriComplete = json.string("verification_uri_complete"),
                qrCodeUrl = json.string("qr_code_url"),
                expiresInMs = expiresInS * 1000,
                intervalMs = intervalS?.times(1000) ?: DEFAULT_INTERVAL_MS,
            )
        )
    }

    override suspend fun poll(deviceCode: String): PollResult {
        val endpoints = endpointsProvider.deviceFlowEndpoints()
            ?: return PollResult.Error("Cannot reach the sign-in server")

        val form = FormBody.Builder()
            .add("grant_type", DEVICE_CODE_GRANT_TYPE)
            .add("device_code", deviceCode)
            .add("client_id", oidcConfig.clientId)
            .build()

        val (code, json) = try {
            post(endpoints.tokenEndpoint, form)
        } catch (e: IOException) {
            logger.debug("poll() token request failed: ${e.javaClass.simpleName}")
            return PollResult.Error("Cannot reach the sign-in server")
        }

        if (code in 200..299) {
            val idToken = json?.string("id_token")
                ?: return PollResult.Error("No ID token received")
            logger.info("poll() device code approved")
            return PollResult.Approved(
                idToken = idToken,
                refreshToken = json.string("refresh_token"),
                userHandle = userHandleFromIdToken(idToken),
            )
        }

        if (code == 429) return PollResult.SlowDown
        return when (val error = json?.string("error")) {
            "authorization_pending" -> PollResult.Pending
            "slow_down" -> PollResult.SlowDown
            "expired_token" -> PollResult.Expired
            "access_denied" -> PollResult.Denied
            else -> {
                logger.warn("poll() token endpoint error: ${error ?: "HTTP $code"}")
                PollResult.Error(json?.string("error_description") ?: error ?: "HTTP $code")
            }
        }
    }

    /** Returns the HTTP status and the JSON body, if the body is a JSON object. */
    private suspend fun post(url: String, form: FormBody): Pair<Int, JsonObject?> {
        val request = Request.Builder()
            .url(url)
            .header("Accept", "application/json")
            .post(form)
            .build()
        return httpClient.newCall(request).await().use { response ->
            val body = response.body?.string().orEmpty()
            response.code to runCatching { Json.parseToJsonElement(body).jsonObject }.getOrNull()
        }
    }

    private fun JsonObject.string(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content?.takeIf { it.isNotEmpty() }

    private fun JsonObject.long(key: String): Long? = (this[key] as? JsonPrimitive)?.longOrNull

    private companion object {
        /** Used only if the server omits the mandatory `expires_in`. */
        const val DEFAULT_EXPIRES_IN_S = 600L
    }
}

/** Enqueues the call and cancels it when the coroutine is cancelled. */
private suspend fun Call.await(): Response = suspendCancellableCoroutine { continuation ->
    enqueue(object : Callback {
        override fun onResponse(call: Call, response: Response) {
            continuation.resume(response) { _, value, _ -> value.close() }
        }

        override fun onFailure(call: Call, e: IOException) {
            continuation.resumeWithException(e)
        }
    })
    continuation.invokeOnCancellation { runCatching { cancel() } }
}
