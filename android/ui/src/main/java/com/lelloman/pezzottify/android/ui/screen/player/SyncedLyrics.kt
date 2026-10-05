package com.lelloman.pezzottify.android.ui.screen.player

data class LyricLine(val timeMs: Long, val text: String)

private val timestamp = Regex("\\[(\\d+):([0-5]\\d)(?:\\.(\\d{1,3}))?\\]")
private val offsetTag = Regex("\\[offset:([+-]?\\d+)\\]", RegexOption.IGNORE_CASE)
private val metadataTag = Regex("\\[[^\\]]*\\]")

fun parseSyncedLyrics(text: String?): List<LyricLine> {
    if (text.isNullOrBlank()) return emptyList()
    val offset = offsetTag.find(text)?.groupValues?.get(1)?.toLongOrNull() ?: 0L
    return text.lineSequence().flatMap { raw ->
        val content = raw.replace(metadataTag, "").trim()
        timestamp.findAll(raw).mapNotNull { match ->
            val minutes = match.groupValues[1].toLongOrNull() ?: return@mapNotNull null
            if (minutes > 1_000_000L || offset !in -86_400_000L..86_400_000L) return@mapNotNull null
            val milliseconds = match.groupValues[3].padEnd(3, '0').toLong()
            LyricLine((minutes * 60_000 + match.groupValues[2].toLong() * 1000 + milliseconds - offset).coerceAtLeast(0), content)
        }
    }.sortedBy { it.timeMs }.toList()
}

fun activeLyricIndex(lines: List<LyricLine>, positionMs: Long): Int =
    lines.indexOfLast { it.timeMs <= positionMs }
