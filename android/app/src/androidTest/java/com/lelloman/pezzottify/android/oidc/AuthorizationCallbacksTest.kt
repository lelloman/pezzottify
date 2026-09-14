package com.lelloman.pezzottify.android.oidc

import android.net.Uri
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import net.openid.appauth.AuthorizationRequest
import net.openid.appauth.AuthorizationResponse
import net.openid.appauth.AuthorizationServiceConfiguration
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AuthorizationCallbacksTest {
    private fun request() = AuthorizationRequest.Builder(
        AuthorizationServiceConfiguration(Uri.parse("https://auth.example.test/authorize"), Uri.parse("https://auth.example.test/token")),
        "synthetic-client", "code", Uri.parse("com.lelloman.pezzottify.android:/oauth2redirect")
    ).setScopes("openid", "profile").build()

    @Test fun restoredRequestPreservesStatePkceAndTokenExchange() {
        val original = request()
        val restored = AuthorizationRequest.jsonDeserialize(original.jsonSerializeString())
        val callback = Uri.parse("${original.redirectUri}?code=synthetic-code&state=${original.state}")
        AuthorizationCallbacks.validate(callback, restored)
        val response = AuthorizationResponse.Builder(restored).fromUri(callback).build()
        AuthorizationCallbacks.validate(response, restored)
        val exchange = response.createTokenExchangeRequest()
        assertEquals(original.codeVerifier, exchange.codeVerifier)
        assertEquals(original.redirectUri, exchange.redirectUri)
        assertEquals(original.clientId, exchange.clientId)
        assertEquals(original.nonce, restored.nonce)
    }

    @Test fun wrongStateDuplicateStateAndForeignRedirectFailClosed() {
        val request = request()
        val valid = "${request.redirectUri}?code=synthetic-code&state=${request.state}"
        listOf(valid.replace("synthetic-code&state=${request.state}", "synthetic-code&state=wrong"),
            "$valid&state=${request.state}", "$valid&error=access_denied", "$valid#fragment",
            valid.replace("com.lelloman.pezzottify.android", "com.evil.app"),
            "${request.redirectUri}?error=access_denied&state=wrong").forEach {
            assertTrue(runCatching { AuthorizationCallbacks.validate(Uri.parse(it), request) }.isFailure)
        }
        AuthorizationCallbacks.validate(Uri.parse("${request.redirectUri}?error=access_denied&state=${request.state}"), request)
        val other = request()
        val substituted = AuthorizationResponse.Builder(other).fromUri(Uri.parse("${other.redirectUri}?code=synthetic-code&state=${other.state}")).build()
        assertTrue(runCatching { AuthorizationCallbacks.validate(substituted, request) }.isFailure)
    }

    @Test fun disabledOrMissingAuthenticatorUsesBrowserFallback() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        assertNull(AuthenticatorLauncher(context, false, "com.lelloman.authenticator", "ab".repeat(32)).launch(request(), "https://auth.example.test"))
        assertNull(AuthenticatorLauncher(context, true, "com.example.missing.authenticator", "ab".repeat(32)).launch(request(), "https://auth.example.test"))
    }
}
