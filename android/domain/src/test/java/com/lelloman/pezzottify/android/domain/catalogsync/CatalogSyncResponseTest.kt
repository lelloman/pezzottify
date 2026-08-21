package com.lelloman.pezzottify.android.domain.catalogsync

import com.google.common.truth.Truth.assertThat
import kotlinx.serialization.json.Json
import org.junit.Test

class CatalogSyncResponseTest {

    @Test
    fun `decodes catalog page metadata`() {
        val response = Json.decodeFromString<CatalogSyncResponse>(
            """{
                "events": [],
                "current_seq": 9,
                "has_more": true,
                "next_since": 4
            }""".trimIndent(),
        )

        assertThat(response.currentSeq).isEqualTo(9)
        assertThat(response.hasMore).isTrue()
        assertThat(response.nextSince).isEqualTo(4)
    }

    @Test
    fun `legacy response remains a complete single page`() {
        val response = Json.decodeFromString<CatalogSyncResponse>(
            """{
                "events": [],
                "current_seq": 9
            }""".trimIndent(),
        )

        assertThat(response.hasMore).isFalse()
        assertThat(response.nextSince).isEqualTo(9)
    }
}
