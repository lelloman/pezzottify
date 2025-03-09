package com.lelloman.pezzottify.android.remoteapi.response

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.response.ActivityPeriod
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import org.junit.Test

class ActivityPeriodSerializerTest {

    @Test
    fun `serializes activity period decade`() {
        val decade = ActivityPeriod.Decade(1234)
        val json = Json.encodeToString(ActivityPeriod.serializer(), decade)

        assertThat(json).isEqualTo("{\"Decade\":1234}")
        assertThat(Json.decodeFromString(ActivityPeriod.serializer(), json)).isEqualTo(decade)
    }

    @Test
    fun `serializes activity period timespan`() {
        val decade = ActivityPeriod.Timespan(123, 321)
        val json = Json.encodeToString(ActivityPeriod.serializer(), decade)

        assertThat(json).isEqualTo("{\"Timespan\":{\"start_year\":123,\"end_year\":321}}")
        assertThat(Json.decodeFromString(ActivityPeriod.serializer(), json)).isEqualTo(decade)
    }
}