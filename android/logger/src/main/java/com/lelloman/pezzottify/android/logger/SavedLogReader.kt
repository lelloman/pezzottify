package com.lelloman.pezzottify.android.logger

import java.io.File
import java.text.SimpleDateFormat
import java.util.Locale

/** Searches only the app's rolling log files, in oldest-to-newest order. */
class SavedLogReader(private val directory: File) {
    data class Entry(val timestamp: String, val level: String, val tag: String, val message: String, val truncated: Boolean)
    data class Page(val entries: List<Entry>, val nextOffset: Int?, val files: List<String>)

    fun read(offset: Int = 0, limit: Int = 100, from: String? = null, to: String? = null,
             minimumLevel: String = "INFO", tag: String? = null, text: String? = null): Page {
        require(offset >= 0 && limit in 1..100)
        require(minimumLevel in levels)
        from?.let(::validateDate)
        to?.let(::validateDate)
        require(from == null || to == null || from <= to) { "from must precede to" }
        val files = directory.listFiles()?.filter { it.isFile && fileName.matches(it.name) }
            ?.sortedByDescending { it.name } ?: emptyList()
        val entries = mutableListOf<Entry>()
        var skipped = 0
        var budget = 24_000
        for (file in files) {
            file.bufferedReader().use { reader ->
                var header: MatchResult? = null
                var message = StringBuilder()
                var truncated = false
                var textMatches = text == null
                fun emit(): Boolean {
                    val h = header ?: return true
                    val entry = Entry(h.groupValues[1], h.groupValues[2], h.groupValues[3], message.toString(), truncated)
                    if ((from != null && entry.timestamp < from) || (to != null && entry.timestamp > to) ||
                        levels.indexOf(entry.level) < levels.indexOf(minimumLevel) ||
                        (tag != null && entry.tag != tag) || !textMatches) return true
                    if (skipped++ < offset) return true
                    val cost = entry.message.length + entry.tag.length + 512
                    if (entries.size == limit || cost > budget) return false
                    entries.add(entry)
                    budget -= cost
                    return true
                }
                reader.lineSequence().forEach { line ->
                    val match = headerPattern.matchEntire(line)
                    if (match != null) {
                        if (!emit()) return Page(entries, offset + entries.size, files.map { it.name })
                        header = match
                        message = StringBuilder(match.groupValues[4].take(4096))
                        truncated = match.groupValues[4].length > 4096
                        textMatches = text == null || match.groupValues[4].contains(text, ignoreCase = true)
                    } else if (header != null) {
                        if (text != null && line.contains(text, ignoreCase = true)) textMatches = true
                        val remaining = 4096 - message.length
                        val addition = "\n$line"
                        message.append(addition.take(remaining))
                        truncated = truncated || addition.length > remaining
                    }
                }
                if (!emit()) return Page(entries, offset + entries.size, files.map { it.name })
            }
        }
        return Page(entries, null, files.map { it.name })
    }

    private fun validateDate(value: String) {
        require(datePattern.matches(value)) { "Use yyyy-MM-dd HH:mm:ss.SSS in the log's device-local time" }
        val parser = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US).apply { isLenient = false }
        require(runCatching { parser.parse(value) }.getOrNull() != null) { "Invalid date" }
    }

    companion object {
        private val levels = listOf("INFO", "WARN", "ERROR")
        private val fileName = Regex("pezzottify_[0-4]\\.log")
        private val datePattern = Regex("\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}\\.\\d{3}")
        private val headerPattern = Regex("^\\[([0-9-]+ [0-9:.]+)] \\[(INFO|WARN|ERROR)] \\[([^]]*)] (.*)$")
    }
}
