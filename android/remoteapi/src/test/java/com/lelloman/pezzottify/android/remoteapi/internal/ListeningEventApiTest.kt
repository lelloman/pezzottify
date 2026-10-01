package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import okhttp3.*
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Test

class ListeningEventApiTest {
    @Test fun `empty 400 preserves status so rejected history is not retried`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val factory = object : OkHttpClientFactory() {
            override fun createBuilder(baseUrl: String) = OkHttpClient.Builder().addInterceptor { chain ->
                Response.Builder().request(chain.request()).protocol(Protocol.HTTP_1_1)
                    .code(400).message("").body("".toResponseBody()).build()
            }
        }
        val api = RemoteApiClientImpl(
            object : RemoteApiClient.HostUrlProvider { override val hostUrl = MutableStateFlow("https://example.test/") },
            factory, object : RemoteApiCredentialsProvider { override val authToken = "token" }, scope)
        try {
            val result = api.recordListeningEvent(ListeningEventSyncData("track", "session-id", 1000L,
                1010L, 10, 300, 0, 0, "album")) as RemoteApiResponse.Error.Unknown
            assertThat(result.httpStatus).isEqualTo(400)
        } finally { scope.cancel() }
    }
}
