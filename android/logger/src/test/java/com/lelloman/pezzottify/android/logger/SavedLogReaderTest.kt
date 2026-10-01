package com.lelloman.pezzottify.android.logger

import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class SavedLogReaderTest {
    @get:Rule val folder = TemporaryFolder()
    private fun file(name: String, text: String) = folder.newFile(name).writeText(text)

    @Test fun `filters dates severity tag and text and keeps stack traces`() {
        file("pezzottify_1.log", "[2026-10-01 10:00:00.000] [INFO] [Auth] Refresh success\n")
        file("pezzottify_0.log", "[2026-10-01 11:00:00.000] [WARN] [Auth] Refresh failed\njava.io.IOException\n at example\n[2026-10-01 12:00:00.000] [ERROR] [Player] Playback failed\n")
        val result = SavedLogReader(folder.root).read(from = "2026-10-01 10:30:00.000",
            to = "2026-10-01 11:30:00.000", minimumLevel = "WARN", tag = "Auth", text = "REFRESH")
        assertEquals(1, result.entries.size)
        assertTrue(result.entries.single().message.contains("java.io.IOException\n at example"))
        assertNull(result.nextOffset)
    }

    @Test fun `paginates filtered results oldest first and excludes other files`() {
        file("secret.log", "[2026-10-01 09:00:00.000] [INFO] [Auth] secret\n")
        file("pezzottify_1.log", "[2026-10-01 10:00:00.000] [INFO] [Auth] first\n")
        file("pezzottify_0.log", "[2026-10-01 11:00:00.000] [INFO] [Auth] second\n")
        val reader = SavedLogReader(folder.root)
        val first = reader.read(limit = 1)
        assertEquals("first", first.entries.single().message)
        assertEquals(1, first.nextOffset)
        val second = reader.read(offset = first.nextOffset!!, limit = 1)
        assertEquals("second", second.entries.single().message)
        assertNull(second.nextOffset)
    }

    @Test fun `bounds messages and page size`() {
        file("pezzottify_0.log", (1..20).joinToString("\n") {
            "[2026-10-01 11:00:00.000] [ERROR] [Auth] " + "x".repeat(5000)
        })
        val result = SavedLogReader(folder.root).read()
        assertTrue(result.entries.all { it.truncated && it.message.length == 4096 })
        assertTrue(result.entries.sumOf { it.message.length } < 24_000)
        assertNotNull(result.nextOffset)
    }

    @Test(expected = IllegalArgumentException::class)
    fun `rejects invalid dates`() {
        SavedLogReader(folder.root).read(from = "2026-02-30 00:00:00.000")
    }

    @Test fun `searches stack trace beyond returned message cap`() {
        file("pezzottify_0.log", "[2026-10-01 11:00:00.000] [ERROR] [Auth] " +
            "x".repeat(5000) + "\nInvalidGrantException\n")
        val result = SavedLogReader(folder.root).read(text = "InvalidGrant")
        assertEquals(1, result.entries.size)
        assertTrue(result.entries.single().truncated)
    }

    @Test fun `missing logs returns empty page`() {
        val result = SavedLogReader(folder.root).read()
        assertTrue(result.entries.isEmpty())
        assertNull(result.nextOffset)
    }
}
