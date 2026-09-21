package com.lelloman.pezzottify.android.logger

import org.junit.Assert.*
import org.junit.Test

class DiagnosticLogBufferTest {
    @Test fun `capture is dormant until a session and cleared on stop or replacement`() {
        val buffer = DiagnosticLogBuffer()
        var session: String? = null
        buffer.configure { session }
        buffer.record(LogLevel.Info, "tag", "before")
        assertTrue(buffer.read().entries.isEmpty())
        session = "one"
        buffer.record(LogLevel.Info, "tag", "during")
        assertEquals("during", buffer.read().entries.single().message)
        session = null
        buffer.refreshSession()
        assertNull(buffer.read().sessionId)
        assertTrue(buffer.read().entries.isEmpty())
        session = "two"
        buffer.record(LogLevel.Info, "tag", "new")
        session = "three"
        assertTrue(buffer.read().entries.isEmpty())
    }

    @Test fun `raw secrets and throwable text are retained without sanitizing`() {
        val buffer = DiagnosticLogBuffer()
        buffer.configure { "session" }
        val throwable = IllegalStateException("password=secret").apply {
            // Runner/JDK stack depth must not decide whether this fixture is truncated.
            stackTrace = arrayOf(StackTraceElement("AuthClient", "login", "AuthClient.kt", 42))
        }
        buffer.record(LogLevel.Error, "auth", "Authorization: Bearer raw-token", throwable)
        val entry = buffer.read().entries.single()
        assertTrue(entry.message.contains("Bearer raw-token"))
        assertTrue(entry.message.contains("password=secret"))
        assertTrue(entry.message.contains("IllegalStateException"))
        assertTrue(entry.message.contains("AuthClient.login(AuthClient.kt:42)"))
        assertFalse(entry.truncated)
    }

    @Test fun `long throwable stack traces are bounded and marked truncated`() {
        val buffer = DiagnosticLogBuffer()
        buffer.configure { "session" }
        val throwable = IllegalStateException("password=secret").apply {
            stackTrace = Array(200) { StackTraceElement("AuthClient", "login", "AuthClient.kt", it + 1) }
        }
        buffer.record(LogLevel.Error, "auth", "Authorization: Bearer raw-token", throwable)
        val entry = buffer.read().entries.single()
        assertEquals(4096, entry.message.length)
        assertTrue(entry.message.startsWith("Authorization: Bearer raw-token\njava.lang.IllegalStateException: password=secret"))
        assertTrue(entry.message.contains("AuthClient.login(AuthClient.kt:1)"))
        assertTrue(entry.truncated)
    }

    @Test fun `capacity message size filters and cursor pagination are bounded`() {
        val buffer = DiagnosticLogBuffer(3)
        buffer.configure { "session" }
        buffer.record(LogLevel.Debug, "a", "evicted")
        buffer.record(LogLevel.Info, "b", "ignored")
        buffer.record(LogLevel.Warn, "a", "warning")
        buffer.record(LogLevel.Error, "a", "x".repeat(5000))
        val first = buffer.read(limit = 1, minimumLevel = LogLevel.Warn, tag = "a")
        assertEquals(2L, first.oldestAvailableId)
        assertTrue(first.hasMore)
        assertEquals("warning", first.entries.single().message)
        val last = buffer.read(after = first.nextCursor, minimumLevel = LogLevel.Warn, tag = "a")
        assertEquals(4096, last.entries.single().message.length)
        assertTrue(last.entries.single().truncated)
        assertFalse(last.hasMore)
        assertTrue(buffer.read(after = last.nextCursor).entries.isEmpty())
    }

    @Test fun `logger created while dormant captures debug after activation and still delegates`() {
        val buffer = DiagnosticLogBuffer()
        var session: String? = null
        buffer.configure { session }
        val forwarded = mutableListOf<String>()
        val delegate = object : Logger {
            override fun debug(message: String, throwable: Throwable?) { forwarded += message }
            override fun info(message: String, throwable: Throwable?) { forwarded += message }
            override fun warn(message: String, throwable: Throwable?) { forwarded += message }
            override fun error(message: String, throwable: Throwable?) { forwarded += message }
        }
        val logger = buffer.logger("tag", delegate)
        logger.debug("before")
        session = "active"
        logger.debug("captured")
        assertEquals("captured", buffer.read().entries.single().message)
        session = null
        logger.info("after")
        assertEquals(listOf("before", "captured", "after"), forwarded)
        assertTrue(buffer.read().entries.isEmpty())
    }
}
