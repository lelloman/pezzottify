package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Test

class LyricsApiTest {
    @Test fun `lyrics endpoint handles JSON null found lyrics and HTTP failures`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var body = "null"
        var code = 200
        val factory = object : OkHttpClientFactory() {
            override fun createBuilder(baseUrl: String) = OkHttpClient.Builder().addInterceptor { chain ->
                assertThat(chain.request().url.encodedPath).isEqualTo("/v1/content/track/t/lyrics")
                assertThat(chain.request().header("Authorization")).isNotEmpty()
                Response.Builder().request(chain.request()).protocol(Protocol.HTTP_1_1)
                    .code(code).message("").body(body.toResponseBody("application/json".toMediaType())).build()
            }
        }
        val api = RemoteApiClientImpl(
            object : RemoteApiClient.HostUrlProvider { override val hostUrl = MutableStateFlow("https://example.test/") },
            factory, object : RemoteApiCredentialsProvider { override val authToken = "token" }, scope)
        try {
            val missing = api.getTrackLyrics("t")
            assertThat(missing).isEqualTo(RemoteApiResponse.Success(null))
            body = """{"track_id":"t","status":"found","provider":"lrclib","plain_lyrics":"Line","synced_lyrics":"[00:01]Line","fetched_at":123,"retry_at":456,"provider_id":42}"""
            val found = api.getTrackLyrics("t") as RemoteApiResponse.Success
            assertThat(found.data?.plainLyrics).isEqualTo("Line")
            assertThat(found.data?.syncedLyrics).isEqualTo("[00:01]Line")
            code = 503
            assertThat(api.getTrackLyrics("t")).isInstanceOf(RemoteApiResponse.Error::class.java)
        } finally { scope.cancel() }
    }
}
