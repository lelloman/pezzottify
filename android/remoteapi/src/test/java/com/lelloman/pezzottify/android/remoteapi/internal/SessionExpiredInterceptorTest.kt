package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.logger.Logger
import io.mockk.mockk
import io.mockk.verify
import okhttp3.Interceptor
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import org.junit.Before
import org.junit.Test

class SessionExpiredInterceptorTest {

    private lateinit var sessionExpiredHandler: SessionExpiredHandler
    private lateinit var logger: Logger
    private lateinit var interceptor: SessionExpiredInterceptor

    @Before
    fun setUp() {
        sessionExpiredHandler = mockk(relaxed = true)
        logger = mockk(relaxed = true)
        interceptor = SessionExpiredInterceptor(sessionExpiredHandler, logger)
    }

    @Test
    fun `triggers handler on 401 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 401
        )

        interceptor.intercept(chain)

        verify(exactly = 1) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `triggers handler on 403 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 403
        )

        interceptor.intercept(chain)

        verify(exactly = 1) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `does not trigger handler on 200 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 200
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `does not trigger handler on 404 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 404
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `does not trigger handler on 500 response`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 500
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips login endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/login",
            responseCode = 403
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips logout endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/logout",
            responseCode = 403
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `skips challenge endpoint on 403`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/auth/challenge",
            responseCode = 403
        )

        interceptor.intercept(chain)

        verify(exactly = 0) { sessionExpiredHandler.onSessionExpired() }
    }

    @Test
    fun `returns response unchanged`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 403
        )

        val result = interceptor.intercept(chain)

        assertThat(result.code).isEqualTo(403)
    }

    @Test
    fun `logs warning on session expiration`() {
        val chain = createChain(
            requestUrl = "http://localhost/v1/content/album/123",
            responseCode = 401
        )

        interceptor.intercept(chain)

        verify { logger.warn(match { it.contains("Session expired") && it.contains("401") }) }
    }

    private fun createChain(requestUrl: String, responseCode: Int): Interceptor.Chain {
        val request = Request.Builder()
            .url(requestUrl)
            .build()

        val response = Response.Builder()
            .request(request)
            .protocol(Protocol.HTTP_1_1)
            .code(responseCode)
            .message("Test")
            .build()

        return object : Interceptor.Chain {
            override fun request(): Request = request
            override fun proceed(request: Request): Response = response
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
}
