package com.lelloman.pezzottify.android.assistant

import com.lelloman.simpleaiassistant.diagnostics.DiagnosticRecorder
import com.lelloman.simpleaiassistant.diagnostics.DiagnosticStorage
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test
import java.security.MessageDigest

class FeedbackContractTest {
    @Test fun `shared server and web fixture recovers without changing attachment content`() = runBlocking {
        val fixture = checkNotNull(javaClass.classLoader!!.getResourceAsStream("assistant-diagnostics-v1.json"))
            .bufferedReader(Charsets.UTF_8).use { it.readText().trim() }
        val expected = Json.parseToJsonElement(fixture).jsonObject
        val owner = MessageDigest.getInstance("SHA-256").digest("server/A".toByteArray())
            .joinToString("") { "%02x".format(it) }
        var persisted: String? = buildJsonObject {
            put("owner", owner)
            put("envelope", expected)
        }.toString()
        val recorder = DiagnosticRecorder(object : DiagnosticStorage {
            override fun read(maxBytes: Int) = persisted
            override fun write(content: String) { persisted = content }
            override fun clear() { persisted = null }
        })
        recorder.setAccount("server/A", true)
        val snapshot = checkNotNull(recorder.snapshot())
        val recovered = Json.parseToJsonElement(snapshot.content).jsonObject
        // captured_at is regenerated; every retained turn/event is unchanged.
        assertEquals(expected["turns"], recovered["turns"])
        assertEquals(expected["schema_version"], recovered["schema_version"])
        assertEquals(snapshot.content.toByteArray(Charsets.UTF_8).size, snapshot.sizeBytes)
        assertTrue(snapshot.sizeBytes > snapshot.content.length)
        recorder.begin("a partial streamed response")
        val partial = Json.parseToJsonElement(checkNotNull(recorder.snapshot()).content).jsonObject
        assertEquals(JsonPrimitive(1), partial["schema_version"])
        assertEquals(JsonPrimitive("running"), partial["turns"]!!.jsonArray.last().jsonObject["status"])
        recorder.setAccount("server/B", false)
        assertNull(persisted)
        assertNull(recorder.snapshot())
        assertFalse(recorder.isCurrent(snapshot))
        assertEquals(expected["turns"], Json.parseToJsonElement(snapshot.content).jsonObject["turns"])
    }
}
