package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationReference
import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationRequest
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.ResponseBody.Companion.toResponseBody
import okio.Buffer
import org.junit.Test

class ContinuationApiTest {
    private fun api(scope: CoroutineScope, onBody: (String) -> String): RemoteApiClient {
        val factory = object : OkHttpClientFactory() {
            override fun createBuilder(baseUrl: String) = OkHttpClient.Builder().addInterceptor { chain ->
                // GET requests have no body: hand the URL over instead.
                val request = chain.request()
                val sentText = request.body?.let { requestBody ->
                    Buffer().also { requestBody.writeTo(it) }.readUtf8()
                } ?: request.url.toString()
                val body = onBody(sentText)
                Response.Builder().request(chain.request()).protocol(Protocol.HTTP_1_1)
                    .code(200).message("").body(body.toResponseBody("application/json".toMediaType())).build()
            }
        }
        return RemoteApiClientImpl(
            object : RemoteApiClient.HostUrlProvider { override val hostUrl = MutableStateFlow("https://example.test/") },
            factory, object : RemoteApiCredentialsProvider { override val authToken = "token" }, scope)
    }

    @Test fun `null knobs are omitted and lists are always sent`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var sent = ""
        try {
            val result = api(scope) { sent = it; """{"track_ids":["x"]}""" }.getContinuationRecommendations(
                ContinuationRequest(
                    contextTrackIds = listOf("a", "b"),
                    sourceTrackIds = listOf("a"),
                    recentTrackIds = listOf("b"),
                    excludeTrackIds = listOf("a", "b"),
                    count = 2,
                ),
            ) as RemoteApiResponse.Success
            val json = Json.parseToJsonElement(sent).jsonObject
            assertThat(json.keys).containsExactly(
                "context_track_ids", "exclude_track_ids", "count", "source_track_ids",
                "source_references", "recent_track_ids", "away",
            )
            assertThat(json["source_references"].toString()).isEqualTo("[]")
            assertThat(json["recent_track_ids"].toString()).isEqualTo("[\"b\"]")
            assertThat(result.data.trackIds).containsExactly("x")
            assertThat(result.data.diagnostics!!.namespaces).isEmpty()
            assertThat(result.data.diagnostics!!.recencyWeight).isNull()
        } finally { scope.cancel() }
    }

    @Test fun `steering fields and diagnostics round trip`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var sent = ""
        try {
            val result = api(scope) {
                sent = it
                """{"track_ids":["y"],"recency_weight":0.2,"progress":0.5,"namespaces":[{"namespace":"musicfm.mean.v1","weight":1.0,"source_to_destination":0.1,"query_to_source":0.7,"query_to_destination":null,"destination_components":[{"entity_type":"artist","entity_id":"a1","similarity":0.42},{"entity_type":"concept","entity_id":"audioset:Jazz","similarity":null}]}]}"""
            }.getContinuationRecommendations(
                ContinuationRequest(
                    sourceReferences = listOf(ContinuationReference("album", "al1", 1.0)),
                    recencyWeight = 0.3,
                    destination = listOf(
                        ContinuationReference("artist", "a1"),
                        ContinuationReference("concept", "audioset:Jazz", 0.5),
                    ),
                    progress = 0.5,
                    criteria = listOf(ContinuationRequest.Criterion("musicfm.mean.v1", 1.0)),
                    diversity = 0.4,
                    randomness = 0.0,
                    mode = "similar",
                    away = listOf(ContinuationReference("track", "t9", 2.0)),
                ),
            ) as RemoteApiResponse.Success
            val json = Json.parseToJsonElement(sent).jsonObject
            assertThat(json["source_references"].toString()).isEqualTo("""[{"entity_type":"album","entity_id":"al1","weight":1.0}]""")
            assertThat(json["destination"].toString()).isEqualTo(
                """[{"entity_type":"artist","entity_id":"a1"},{"entity_type":"concept","entity_id":"audioset:Jazz","weight":0.5}]"""
            )
            assertThat(json["recency_weight"].toString()).isEqualTo("0.3")
            assertThat(json["progress"].toString()).isEqualTo("0.5")
            assertThat(json["criteria"].toString()).isEqualTo("""[{"namespace":"musicfm.mean.v1","weight":1.0}]""")
            assertThat(json["diversity"].toString()).isEqualTo("0.4")
            assertThat(json["randomness"].toString()).isEqualTo("0.0")
            assertThat(json["mode"].toString()).isEqualTo("\"similar\"")
            assertThat(json["away"].toString()).isEqualTo("""[{"entity_type":"track","entity_id":"t9","weight":2.0}]""")
            val diagnostics = result.data.diagnostics!!
            assertThat(diagnostics.recencyWeight).isEqualTo(0.2)
            assertThat(diagnostics.progress).isEqualTo(0.5)
            assertThat(diagnostics.namespaces.single().queryToSource).isEqualTo(0.7)
            assertThat(diagnostics.namespaces.single().queryToDestination).isNull()
            val components = diagnostics.namespaces.single().destinationComponents
            assertThat(components.map { it.entityId }).containsExactly("a1", "audioset:Jazz").inOrder()
            assertThat(components[0].similarity).isEqualTo(0.42)
            assertThat(components[1].similarity).isNull()
        } finally { scope.cancel() }
    }

    @Test fun `no destination is omitted from the request`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var sent = ""
        try {
            api(scope) { sent = it; """{"track_ids":[]}""" }
                .getContinuationRecommendations(ContinuationRequest(sourceTrackIds = listOf("a")))
            assertThat(Json.parseToJsonElement(sent).jsonObject.containsKey("destination")).isFalse()
        } finally { scope.cancel() }
    }

    @Test fun `concepts are fetched with query, family and clamped limit`() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var url = ""
        try {
            val result = api(scope) {
                url = it
                """{"concepts":[{"id":"audioset:Piano","family":"instrument","label":"Piano","example_count":200}]}"""
            }.getConcepts(query = " pia ", family = "instrument", limit = 9999) as RemoteApiResponse.Success
            val parsed = okhttp3.HttpUrl.Builder().scheme("https").host("x").build().resolve(url)!!
            assertThat(parsed.encodedPath).isEqualTo("/v1/content/concepts")
            assertThat(parsed.queryParameter("q")).isEqualTo("pia")
            assertThat(parsed.queryParameter("family")).isEqualTo("instrument")
            assertThat(parsed.queryParameter("limit")).isEqualTo("500")
            val concept = result.data.concepts.single()
            assertThat(concept.label).isEqualTo("Piano")
            assertThat(concept.exampleCount).isEqualTo(200)
        } finally { scope.cancel() }
    }
}
