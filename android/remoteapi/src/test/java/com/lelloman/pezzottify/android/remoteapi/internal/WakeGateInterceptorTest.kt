package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import java.io.IOException
import java.util.concurrent.TimeUnit
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Test

class WakeGateInterceptorTest {
    private fun json(body: String, code: Int = 200) = MockResponse().setResponseCode(code)
        .setHeader("Content-Type", "application/json").setBody(body)

    @Test fun `foreground waits for wake then sends mutation once with credentials`() {
        MockWebServer().use { server ->
            server.enqueue(json("""{"state":"sleeping"}"""))
            server.enqueue(json("""{"state":"ready"}"""))
            server.enqueue(MockResponse().setBody("saved"))
            val client = OkHttpClientFactory().createBuilder(server.url("/").toString()).build()
            val request = Request.Builder().url(server.url("/v1/playlist"))
                .header("Authorization", "Bearer existing").post("mutation".toRequestBody()).build()
            client.newCall(request).execute().use { assertThat(it.body!!.string()).isEqualTo("saved") }
            assertThat(server.takeRequest().path).isEqualTo("/_power/status")
            val wake = server.takeRequest()
            assertThat(wake.path).isEqualTo("/_power/wake")
            assertThat(wake.method).isEqualTo("POST")
            val mutation = server.takeRequest()
            assertThat(mutation.body.readUtf8()).isEqualTo("mutation")
            assertThat(mutation.getHeader("Authorization")).isEqualTo("Bearer existing")
            assertThat(server.requestCount).isEqualTo(3)
        }
    }

    @Test fun `passive requests never call wake and keep sleeping response`() {
        MockWebServer().use { server ->
            server.enqueue(json("""{"code":"SERVER_SLEEPING"}""", 503))
            val client = OkHttpClientFactory().createBuilder(server.url("/").toString()).build()
            val request = Request.Builder().url(server.url("/v1/sync/state"))
                .header("X-Homelab-Wake", "passive").build()
            client.newCall(request).execute().use { assertThat(it.code).isEqualTo(503) }
            assertThat(server.takeRequest().path).isEqualTo("/v1/sync/state")
            assertThat(server.requestCount).isEqualTo(1)
        }
    }

    @Test fun `failed wake never dispatches the original mutation`() {
        MockWebServer().use { server ->
            server.enqueue(json("""{"state":"sleeping"}"""))
            server.enqueue(json("""{"code":"WAKE_TIMEOUT"}""", 503))
            val client = OkHttpClientFactory().createBuilder(server.url("/").toString()).build()
            val request = Request.Builder().url(server.url("/v1/playlist")).post("mutation".toRequestBody()).build()
            try {
                client.newCall(request).execute().close()
                throw AssertionError("Expected wake failure")
            } catch (_: IOException) { }
            assertThat(server.takeRequest().path).isEqualTo("/_power/status")
            assertThat(server.takeRequest().path).isEqualTo("/_power/wake")
            assertThat(server.takeRequest(100, TimeUnit.MILLISECONDS)).isNull()
        }
    }
}
