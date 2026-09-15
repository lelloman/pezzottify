package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.ResponseBody.Companion.toResponseBody
import okio.Buffer
import org.junit.Test

class FeedbackApiTest {
    @Test fun `upload binds destination credentials and immutable payload without general interceptors`() = runBlocking {
        val scope=CoroutineScope(SupervisorJob()+Dispatchers.Default)
        val requests=mutableListOf<Request>()
        val bodies=mutableListOf<String>()
        var generalCalls=0
        val factory=object:OkHttpClientFactory() {
            override fun createBuilder(baseUrl:String)=OkHttpClient.Builder().addInterceptor {chain->
                val request=chain.request();requests+=request
                val buffer=Buffer();request.body!!.writeTo(buffer);bodies+=buffer.readUtf8()
                Response.Builder().request(request).protocol(Protocol.HTTP_1_1).code(429).message("Limited")
                    .header("Retry-After","60").body("{}".toResponseBody("application/json".toMediaType())).build()
            }
        }
        val api=RemoteApiClientImpl(object:RemoteApiClient.HostUrlProvider {override val hostUrl=MutableStateFlow("https://different.example/")},factory,
            object:RemoteApiCredentialsProvider {override val authToken="different-token"},scope,
            interceptors=listOf(Interceptor {chain->generalCalls++;chain.proceed(chain.request())}))
        try {
            val report=FeedbackReport("immutable-id","bug","assistant",null,"description",clientVersion="test",deviceInfo=null,attachments=listOf(FeedbackAttachment("assistant","exact 🌍 snapshot")))
            repeat(2) {
                val result=api.submitFeedback(report,"https://selected.example/","selected-token") as FeedbackResult.Failed
                assertThat(result.status).isEqualTo(429);assertThat(result.retryAfterSeconds).isEqualTo(60)
            }
            assertThat(requests.map {it.url.host}).containsExactly("selected.example","selected.example")
            assertThat(requests.first().header("Authorization")).isEqualTo("Bearer selected-token")
            assertThat(bodies[0]).isEqualTo(bodies[1])
            assertThat(bodies[0]).contains("client_request_id")
            assertThat(bodies[0]).contains("exact 🌍 snapshot")
            assertThat(generalCalls).isEqualTo(0)
        } finally {scope.cancel()}
    }
}
