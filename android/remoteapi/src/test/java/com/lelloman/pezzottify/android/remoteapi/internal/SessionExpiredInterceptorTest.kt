package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.logger.Logger
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.verify
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Before
import org.junit.Test

class SessionExpiredInterceptorTest {

    private lateinit var sessionExpiredHandler: SessionExpiredHandler
    private lateinit var tokenRefresher: TokenRefresher
    private lateinit var logger: Logger
    private lateinit var interceptor: SessionExpiredInterceptor

    @Before
    fun setUp() {
        sessionExpiredHandler = mockk(relaxed = true)
        tokenRefresher = mockk(relaxed = true)
        logger = mockk(relaxed = true)
        // Default: no refresh token available
        coEvery { tokenRefresher.refreshTokens() } returns TokenRefresher.RefreshResult.NotAvailable
        interceptor = SessionExpiredInterceptor(sessionExpiredHandler, tokenRefresher, logger)
    }

    @Test
    fun `triggers handler on 401 response when refresh not available`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 401
        )

        interceptor.intercept(chain)

        coVerify(exactly = 1) { tokenRefresher.refreshTokens() }
        verify(exactly = 1) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `triggers handler on 403 response when refresh not available`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 403
        )

        interceptor.intercept(chain)

        coVerify(exactly = 1) { tokenRefresher.refreshTokens() }
        verify(exactly = 1) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `triggers handler on 401 when refresh fails`() {
        coEvery { tokenRefresher.refreshTokens() } returns TokenRefresher.RefreshResult.Failed("Token expired")
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 401
        )

        interceptor.intercept(chain)

        coVerify(exactly = 1) { tokenRefresher.refreshTokens() }
        verify(exactly = 1) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `retries request on successful refresh`() {
        coEvery { tokenRefresher.refreshTokens() } returns TokenRefresher.RefreshResult.Success("new-token")
        var callCount = 0
        val chain = createChainWithRetry(
            requestUrl = "http://localhost/v1/content/album/123",
            firstResponseCode = 401,
            retryResponseCode = 200
        ) { callCount++ }

        val result = interceptor.intercept(chain)

        coVerify(exactly = 1) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
        assertThat(callCount).isEqualTo(2) // Original + retry
        assertThat(result.code).isEqualTo(200)
    }

    @Test
    fun `retried request uses new auth token`() {
        coEvery { tokenRefresher.refreshTokens() } returns TokenRefresher.RefreshResult.Success("new-token-123")
        var retryRequest: Request? = null
        val chain = createChainWithRetry(
            requestUrl = "http://localhost/v1/content/album/123",
            firstResponseCode = 401,
            retryResponseCode = 200
        ) { request -> retryRequest = request }

        interceptor.intercept(chain)

        assertThat(retryRequest?.header("Authorization")).isEqualTo("new-token-123")
    }

    @Test
    fun `does not trigger handler on 200 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 200
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `does not trigger handler on 404 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 404
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `does not trigger handler on 500 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 500
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips login endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/login",
            responseCode = 403
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips logout endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/logout",
            responseCode = 403
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips challenge endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/challenge",
            responseCode = 403
        )

        interceptor.intercept(chain)

        coVerify(exactly = 0) { tokenRefresher.refreshTokens() }
        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `returns response unchanged when refresh not available`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 403
        )

        val result = interceptor.intercept(chain)

        assertThat(result.code).isEqualTo(403)
    }

    @Test
    fun `logs warning on unauthorized response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 401
        )

        interceptor.intercept(chain)

        verify { logger.warn(match { it.contains("Unauthorized") && it.contains("401") }) }
    }

    private fun createChain(requestUrl: String, responseCode: Int): Interceptor.Chain {
        val request = Request.Builder()
            .url(requestUrl)
            .build()

        return object : Interceptor.Chain {
            override fun request(): Request = request
            override fun proceed(request: Request): Response = buildResponse(request, responseCode)
            override fun connection() = null
            override fun call() = throw UnsupportedOperationException()
            override fun connectTimeoutMillis() = 0
            override fun withConnectTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
            override fun readTimeoutMillis() = 0
            override fun withReadTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
            override fun writeTimeoutMillis() = 0
            override fun withWriteTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
        }
    }

    private fun createChainWithRetry(
        requestUrl: String,
        firstResponseCode: Int,
        retryResponseCode: Int,
        onProceed: (Request) -> Unit = {}
    ): Interceptor.Chain {
        val request = Request.Builder()
            .url(requestUrl)
            .build()

        var callCount = 0

        return object : Interceptor.Chain {
            override fun request(): Request = request
            override fun proceed(request: Request): Response {
                onProceed(request)
                val code = if (callCount++ == 0) firstResponseCode else retryResponseCode
                return buildResponse(request, code)
            }
            override fun connection() = null
            override fun call() = throw UnsupportedOperationException()
            override fun connectTimeoutMillis() = 0
            override fun withConnectTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
            override fun readTimeoutMillis() = 0
            override fun withReadTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
            override fun writeTimeoutMillis() = 0
            override fun withWriteTimeout(timeout: Int, unit: java.util.concurrent.TimeUnit) =
                throw UnsupportedOperationException()
        }
    }

    private fun buildResponse(request: Request, code: Int): Response {
        return Response.Builder()
            .request(request)
            .protocol(Protocol.HTTP_1_1)
            .code(code)
            .message("Test")
            .body("".toResponseBody("text/plain".toMediaType()))
            .build()
    }
}
