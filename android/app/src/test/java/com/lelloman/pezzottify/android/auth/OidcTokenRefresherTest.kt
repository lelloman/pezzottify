package com.lelloman.pezzottify.android.auth

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test
import java.util.concurrent.atomic.AtomicInteger
import kotlin.reflect.KProperty

class OidcTokenRefresherTest {

    private lateinit var authStore: AuthStore
    private lateinit var oidcAuthManager: OidcAuthManager
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var tokenRefresher: OidcTokenRefresher

    private val authStateFlow = MutableStateFlow<AuthState>(AuthState.LoggedOut)

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        oidcAuthManager = mockk(relaxed = true)
        loggerFactory = mockk()

        val logger = mockk<Logger>(relaxed = true)
        every { loggerFactory.getValue(any(), any<KProperty<*>>()) } returns logger

        every { authStore.getAuthState() } returns authStateFlow

        tokenRefresher = OidcTokenRefresher(authStore, oidcAuthManager, loggerFactory)
    }

    private fun createLoggedInState(refreshToken: String? = "refresh-token") = AuthState.LoggedIn(
        userHandle = "user",
        authToken = "old-token",
        refreshToken = refreshToken,
        remoteUrl = "http://localhost:3001"
    )

    @Test
    fun `returns NotAvailable when not logged in`() = runTest {
        authStateFlow.value = AuthState.LoggedOut

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isEqualTo(TokenRefresher.RefreshResult.NotAvailable)
        coVerify(exactly = 0) { oidcAuthManager.refreshTokens(any()) }
    }

    @Test
    fun `returns NotAvailable when no refresh token`() = runTest {
        authStateFlow.value = createLoggedInState(refreshToken = null)

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isEqualTo(TokenRefresher.RefreshResult.NotAvailable)
        coVerify(exactly = 0) { oidcAuthManager.refreshTokens(any()) }
    }

    @Test
    fun `returns Success on successful refresh`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } returns
            OidcAuthManager.RefreshResult.Success(idToken = "new-token", refreshToken = "new-refresh")

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isInstanceOf(TokenRefresher.RefreshResult.Success::class.java)
        assertThat((result as TokenRefresher.RefreshResult.Success).newAuthToken).isEqualTo("new-token")
        coVerify { authStore.storeAuthState(any()) }
    }

    @Test
    fun `returns Failed on refresh failure`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } returns
            OidcAuthManager.RefreshResult.Failed("Token expired")

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isInstanceOf(TokenRefresher.RefreshResult.Failed::class.java)
        assertThat((result as TokenRefresher.RefreshResult.Failed).reason).isEqualTo("Token expired")
    }

    @Test
    fun `returns Failed when refresh response has no new ID token`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } returns
            OidcAuthManager.RefreshResult.Success(idToken = null, refreshToken = "rotated-refresh")

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isEqualTo(TokenRefresher.RefreshResult.Failed("No fresh ID token received"))
        coVerify { authStore.storeAuthState(createLoggedInState(refreshToken = "rotated-refresh")) }
    }

    @Test
    fun `returns RateLimited when OIDC provider rate limits`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } returns
            OidcAuthManager.RefreshResult.RateLimited(retryAfterMs = 60_000L)

        val result = tokenRefresher.refreshTokens()

        assertThat(result).isInstanceOf(TokenRefresher.RefreshResult.RateLimited::class.java)
        assertThat((result as TokenRefresher.RefreshResult.RateLimited).retryAfterMs).isEqualTo(60_000L)
    }

    @Test
    fun `propagates explicit credential rejection`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens(any()) } returns
            OidcAuthManager.RefreshResult.Failed("Revoked", requiresReauthentication = true)

        assertThat(tokenRefresher.refreshTokens()).isEqualTo(
            TokenRefresher.RefreshResult.Failed("Revoked", requiresReauthentication = true)
        )
        coVerify(exactly = 0) { authStore.storeAuthState(any()) }
    }

    @Test
    fun `network exception preserves credentials and permits a later refresh`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens(any()) } throws java.io.IOException("Offline")

        val failed = tokenRefresher.refreshTokens() as TokenRefresher.RefreshResult.Failed
        assertThat(failed.requiresReauthentication).isFalse()
        coVerify(exactly = 0) { authStore.storeAuthState(any()) }

        coEvery { oidcAuthManager.refreshTokens(any()) } returns
            OidcAuthManager.RefreshResult.Success("new-token", "new-refresh")
        assertThat(tokenRefresher.refreshTokens()).isEqualTo(TokenRefresher.RefreshResult.Success("new-token"))
    }

    @Test
    fun `cancelled owner saves rotated credential and shares success with waiters`() = runTest {
        authStateFlow.value = createLoggedInState()
        val entered = kotlinx.coroutines.CompletableDeferred<Unit>()
        val release = kotlinx.coroutines.CompletableDeferred<Unit>()
        coEvery { authStore.storeAuthState(any()) } coAnswers {
            authStateFlow.value = firstArg()
            Result.success(Unit)
        }
        coEvery { oidcAuthManager.refreshTokens(any()) } coAnswers {
            entered.complete(Unit)
            release.await()
            OidcAuthManager.RefreshResult.Success("new-token", "new-refresh")
        }
        val owner = async { tokenRefresher.refreshTokens() }
        entered.await()
        val waiter = async(start = kotlinx.coroutines.CoroutineStart.UNDISPATCHED) {
            tokenRefresher.refreshTokens()
        }
        owner.cancel()
        release.complete(Unit)
        owner.join()
        assertThat(waiter.await()).isEqualTo(TokenRefresher.RefreshResult.Success("new-token"))
        assertThat((authStateFlow.value as AuthState.LoggedIn).refreshToken).isEqualTo("new-refresh")
        coVerify(exactly = 1) { oidcAuthManager.refreshTokens("refresh-token") }

        coEvery { oidcAuthManager.refreshTokens(any()) } returns
            OidcAuthManager.RefreshResult.Success("new-token", "new-refresh")
        assertThat(tokenRefresher.refreshTokens()).isEqualTo(TokenRefresher.RefreshResult.Success("new-token"))
        coVerify(exactly = 1) { oidcAuthManager.refreshTokens("new-refresh") }
    }

    @Test
    fun `refresh completing after logout does not restore session`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens(any()) } coAnswers {
            authStateFlow.value = AuthState.LoggedOut
            OidcAuthManager.RefreshResult.Success("new-token", "new-refresh")
        }
        assertThat(tokenRefresher.refreshTokens()).isInstanceOf(TokenRefresher.RefreshResult.Failed::class.java)
        coVerify(exactly = 0) { authStore.storeAuthState(any()) }
    }

    // --- Request Coalescing Tests ---

    @Test
    fun `refresh without rotation keeps existing refresh credential`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens(any()) } returns
            OidcAuthManager.RefreshResult.Success("new-token", null)
        assertThat(tokenRefresher.refreshTokens()).isEqualTo(TokenRefresher.RefreshResult.Success("new-token"))
        coVerify { authStore.storeAuthState(createLoggedInState().copy(authToken = "new-token")) }
    }

    @Test
    fun `rejection from previous session does not invalidate new sign in`() = runTest {
        authStateFlow.value = createLoggedInState()
        coEvery { oidcAuthManager.refreshTokens(any()) } coAnswers {
            authStateFlow.value = createLoggedInState("another-refresh-token")
            OidcAuthManager.RefreshResult.Failed("invalid_grant", requiresReauthentication = true)
        }
        val result = tokenRefresher.refreshTokens() as TokenRefresher.RefreshResult.Failed
        assertThat(result.requiresReauthentication).isFalse()
        coVerify(exactly = 0) { authStore.storeAuthState(any()) }
    }

    @Test
    fun `coalesces concurrent refresh requests - only one OIDC call`() = runTest {
        authStateFlow.value = createLoggedInState()

        val oidcCallCount = AtomicInteger(0)
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } coAnswers {
            oidcCallCount.incrementAndGet()
            delay(100) // Simulate network delay
            OidcAuthManager.RefreshResult.Success(idToken = "new-token", refreshToken = "new-refresh")
        }

        // Launch 5 concurrent refresh requests
        val results = (1..5).map {
            async { tokenRefresher.refreshTokens() }
        }.awaitAll()

        // All should succeed with the same token
        results.forEach { result ->
            assertThat(result).isInstanceOf(TokenRefresher.RefreshResult.Success::class.java)
            assertThat((result as TokenRefresher.RefreshResult.Success).newAuthToken).isEqualTo("new-token")
        }

        // Only ONE actual OIDC call should have been made
        assertThat(oidcCallCount.get()).isEqualTo(1)
    }

    @Test
    fun `coalesces concurrent refresh requests - all get same failure result`() = runTest {
        authStateFlow.value = createLoggedInState()

        val oidcCallCount = AtomicInteger(0)
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } coAnswers {
            oidcCallCount.incrementAndGet()
            delay(100)
            OidcAuthManager.RefreshResult.Failed("Token expired")
        }

        // Launch 5 concurrent refresh requests
        val results = (1..5).map {
            async { tokenRefresher.refreshTokens() }
        }.awaitAll()

        // All should fail with the same error
        results.forEach { result ->
            assertThat(result).isInstanceOf(TokenRefresher.RefreshResult.Failed::class.java)
            assertThat((result as TokenRefresher.RefreshResult.Failed).reason).isEqualTo("Token expired")
        }

        // Only ONE actual OIDC call should have been made
        assertThat(oidcCallCount.get()).isEqualTo(1)
    }

    @Test
    fun `allows new refresh after previous one completes`() = runTest {
        authStateFlow.value = createLoggedInState()

        val oidcCallCount = AtomicInteger(0)
        coEvery { oidcAuthManager.refreshTokens("refresh-token") } coAnswers {
            val count = oidcCallCount.incrementAndGet()
            OidcAuthManager.RefreshResult.Success(idToken = "token-$count", refreshToken = "refresh")
        }

        // First refresh
        val result1 = tokenRefresher.refreshTokens()
        assertThat(result1).isInstanceOf(TokenRefresher.RefreshResult.Success::class.java)
        assertThat((result1 as TokenRefresher.RefreshResult.Success).newAuthToken).isEqualTo("token-1")

        // Second refresh (should make a new OIDC call)
        val result2 = tokenRefresher.refreshTokens()
        assertThat(result2).isInstanceOf(TokenRefresher.RefreshResult.Success::class.java)
        assertThat((result2 as TokenRefresher.RefreshResult.Success).newAuthToken).isEqualTo("token-2")

        // Two OIDC calls should have been made
        assertThat(oidcCallCount.get()).isEqualTo(2)
    }
}
