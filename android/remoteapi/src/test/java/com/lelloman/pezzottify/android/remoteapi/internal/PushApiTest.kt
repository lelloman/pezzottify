package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import okio.Buffer
import org.junit.Test

class PushApiTest {

    private data class Sent(val method: String, val path: String, val body: String?)

    private fun withApi(code: Int, responseBody: String, block: suspend (RemoteApiClient, () -> Sent) -> Unit) =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            var sent: Sent? = null
            val factory = object : OkHttpClientFactory() {
                override fun createBuilder(baseUrl: String) = OkHttpClient.Builder().addInterceptor { chain ->
                    val request: Request = chain.request()
                    sent = Sent(
                        request.method,
                        request.url.encodedPath,
                        request.body?.let { Buffer().also(it::writeTo).readUtf8() },
                    )
                    Response.Builder().request(request).protocol(Protocol.HTTP_1_1)
                        .code(code).message("")
                        .body(responseBody.toResponseBody("application/json".toMediaType())).build()
                }
            }
            val api = RemoteApiClientImpl(
                object : RemoteApiClient.HostUrlProvider { override val hostUrl = MutableStateFlow("https://example.test/") },
                factory,
                object : RemoteApiCredentialsProvider { override val authToken = "token" },
                scope,
            )
            try {
                block(api) { sent!! }
            } finally {
                scope.cancel()
            }
        }

    @Test
    fun `vapid key is read from public_key`() = withApi(200, """{"public_key":"BAx"}""") { api, sent ->
        val result = api.getPushVapidKey()
        assertThat(result).isEqualTo(RemoteApiResponse.Success("BAx"))
        assertThat(sent()).isEqualTo(Sent("GET", "/v1/push/vapid", null))
    }

    @Test
    fun `registration sends endpoint, keys and device id`() = withApi(200, "") { api, sent ->
        val result = api.putPushRegistration("https://push.example/e1", "p256", "auth", "device-1")
        assertThat(result).isInstanceOf(RemoteApiResponse.Success::class.java)
        assertThat(sent().method).isEqualTo("PUT")
        assertThat(sent().path).isEqualTo("/v1/push/registrations")
        val json = Json.parseToJsonElement(sent().body!!).jsonObject
        assertThat(json["endpoint"].toString()).isEqualTo("\"https://push.example/e1\"")
        assertThat(json["p256dh"].toString()).isEqualTo("\"p256\"")
        assertThat(json["auth"].toString()).isEqualTo("\"auth\"")
        assertThat(json["device_id"].toString()).isEqualTo("\"device-1\"")
    }

    @Test
    fun `deletion sends the endpoint in a DELETE body`() = withApi(200, "") { api, sent ->
        api.deletePushRegistration("https://push.example/e1")
        assertThat(sent().method).isEqualTo("DELETE")
        assertThat(sent().path).isEqualTo("/v1/push/registrations")
        assertThat(Json.parseToJsonElement(sent().body!!).jsonObject.keys).containsExactly("endpoint")
    }

    @Test
    fun `503 means push is disabled on the server`() = withApi(503, "") { api, _ ->
        val result = api.getPushVapidKey()
        assertThat(result).isInstanceOf(RemoteApiResponse.Error.Unknown::class.java)
        assertThat((result as RemoteApiResponse.Error.Unknown).httpStatus).isEqualTo(503)
        val put = api.putPushRegistration("https://e", "p", "a", null) as RemoteApiResponse.Error.Unknown
        assertThat(put.httpStatus).isEqualTo(503)
    }
}
