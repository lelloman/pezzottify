package com.lelloman.pezzottify.android.logger

/** Unredacted, bounded app logs. No disk persistence and no capture outside a session. */
class DiagnosticLogBuffer(private val capacity: Int = 1000) {
    data class Entry(
        val id: Long,
        val timestampMs: Long,
        val level: LogLevel,
        val tag: String,
        val message: String,
        val truncated: Boolean,
    )

    data class Page(
        val sessionId: String?,
        val entries: List<Entry>,
        val nextCursor: Long,
        val oldestAvailableId: Long?,
        val hasMore: Boolean,
    )

    init { require(capacity > 0) }
    private var sessionProvider: () -> String? = { null }
    private var sessionId: String? = null
    private var nextId = 1L
    private val entries = ArrayDeque<Entry>()

    @Synchronized
    fun configure(sessionProvider: () -> String?) {
        this.sessionProvider = sessionProvider
        refreshSession()
    }

    /** Also called on SDK session changes so Stop/expiry promptly releases retained logs. */
    @Synchronized
    fun refreshSession() {
        val current = sessionProvider()
        if (current != sessionId) {
            entries.clear()
            sessionId = current
        }
    }

    @Synchronized
    fun record(level: LogLevel, tag: String, message: String, throwable: Throwable? = null) {
        refreshSession()
        if (sessionId == null || level == LogLevel.None) return
        val text = if (throwable == null) message else "$message\n${throwable.stackTraceToString()}"
        if (entries.size == capacity) entries.removeFirst()
        entries.addLast(Entry(nextId++, System.currentTimeMillis(), level, tag.take(256), text.take(4096), text.length > 4096))
    }

    @Synchronized
    fun read(after: Long = 0, limit: Int = 100, minimumLevel: LogLevel = LogLevel.Debug, tag: String? = null): Page {
        require(after >= 0 && limit in 1..100)
        refreshSession()
        val matching = entries.filter { it.id > after && it.level.ordinal >= minimumLevel.ordinal && (tag == null || it.tag == tag) }
        // Reserve room for JSON escaping and the MCP text + structured copies within a 1 MiB frame.
        var remainingCharacters = 24_000
        val page = matching.take(limit).takeWhile {
            remainingCharacters -= it.message.length + it.tag.length + 512
            remainingCharacters >= 0
        }
        val hasMore = matching.size > page.size
        return Page(sessionId, page, if (hasMore) page.last().id else maxOf(after, nextId - 1), entries.firstOrNull()?.id, hasMore)
    }

    internal fun logger(tag: String, delegate: Logger): Logger = object : Logger {
        override fun debug(message: String, throwable: Throwable?) {
            record(LogLevel.Debug, tag, message, throwable)
            delegate.debug(message, throwable)
        }
        override fun info(message: String, throwable: Throwable?) {
            record(LogLevel.Info, tag, message, throwable)
            delegate.info(message, throwable)
        }
        override fun warn(message: String, throwable: Throwable?) {
            record(LogLevel.Warn, tag, message, throwable)
            delegate.warn(message, throwable)
        }
        override fun error(message: String, throwable: Throwable?) {
            record(LogLevel.Error, tag, message, throwable)
            delegate.error(message, throwable)
        }
    }
}
