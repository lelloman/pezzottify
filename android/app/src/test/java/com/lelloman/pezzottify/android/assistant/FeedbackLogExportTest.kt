package com.lelloman.pezzottify.android.assistant

import org.junit.Assert.*
import org.junit.Test

class FeedbackLogExportTest {
    @Test fun `technical export keeps latest bytes without splitting Unicode`() {
        val source="old\n"+"🌍".repeat(300_000)+"latest"
        val result=AndroidFeedback.utf8Tail(source,1024*1024)
        assertTrue(result.toByteArray().size<=1024*1024)
        assertTrue(result.endsWith("latest"))
        assertFalse(result.contains("�"))
        assertEquals("short",AndroidFeedback.utf8Tail("short",1024))
    }
}
