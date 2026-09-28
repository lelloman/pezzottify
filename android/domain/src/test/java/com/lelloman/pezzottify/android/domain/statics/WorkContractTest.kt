package com.lelloman.pezzottify.android.domain.statics

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.toDomain
import kotlinx.serialization.json.Json
import org.junit.Test

@OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
class WorkContractTest {
    private val json = Json { ignoreUnknownKeys = true; namingStrategy = kotlinx.serialization.json.JsonNamingStrategy.SnakeCase }
    private val track = """{"track":{"id":"t","name":"Recording","album_id":"a","disc_number":1,"track_number":1,"duration_ms":1000},"album":{"id":"a","name":"Album","album_type":"album"},"artists":[]}"""

    @Test fun `track resolution exists without enrichment and survives domain mapping`() {
        val response = json.decodeFromString<TrackResponse>(track.dropLast(1) + """,
            "work_resolution":{"track_id":"t","work":{"id":"w","title":"Composition","creators":["Writer"],"kind":"song","created_at":1},"status":"resolved","reason":"match","evaluated_at":1},
            "work_enrichment_status":{"entity_type":"work_resolution","entity_id":"t","status":"completed"}}""")
        assertThat(response.enrichment).isNull()
        assertThat(response.toDomain().workResolution?.work?.title).isEqualTo("Composition")
        assertThat(response.toDomain().workEnrichmentStatus?.status).isEqualTo("completed")
    }

    @Test fun `older track payload has no work`() {
        assertThat(json.decodeFromString<TrackResponse>(track).toDomain().workResolution).isNull()
    }

    @Test fun `work page preserves composition ranges relation direction and empty advancing pages`() {
        val page = json.decodeFromString<WorkPage>("""{
            "work":{"id":"w","title":"Etudes","creators":["Chopin"],"composition_year":"1829–1832","creator_artist_ids":["artist"],"catalog_number":null},
            "relations":[{"work":{"id":"child","title":"Movement"},"relationship_type":"parts","direction":"outgoing","ordering":2}],
            "tracks":[],"next_offset":50,"has_more":true
        }""")
        assertThat(page.work.compositionYear).isEqualTo("1829–1832")
        assertThat(page.work.creatorArtistIds).containsExactly("artist")
        assertThat(page.relations.single().direction).isEqualTo("outgoing")
        assertThat(page.tracks).isEmpty()
        assertThat(page.hasMore).isTrue()
        assertThat(page.nextOffset).isEqualTo(50)
    }
}
