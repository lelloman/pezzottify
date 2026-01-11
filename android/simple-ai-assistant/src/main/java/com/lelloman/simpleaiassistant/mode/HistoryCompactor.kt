package com.lelloman.simpleaiassistant.mode

import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.MessageRole
import java.util.UUID

/**
 * Result of compacting conversation history when switching modes.
 *
 * @param keptMessages Recent messages to keep in the conversation
 * @param contextSummary Optional summary of older messages (can be prepended to system prompt or added as context)
 */
data class CompactedHistory(
    val keptMessages: List<ChatMessage>,
    val contextSummary: String?
)

/**
 * Interface for compacting conversation history when switching modes.
 * Implementations can use various strategies: keep recent N, summarize with LLM, etc.
 */
interface HistoryCompactor {
    /**
     * Compacts the conversation history when switching between modes.
     *
     * @param messages Current conversation messages
     * @param fromMode The mode being switched from
     * @param toMode The mode being switched to
     * @return Compacted history with kept messages and optional summary
     */
    suspend fun compact(
        messages: List<ChatMessage>,
        fromMode: AssistantMode,
        toMode: AssistantMode
    ): CompactedHistory
}

/**
 * Default implementation that keeps the most recent messages.
 * Tool result messages are excluded from the count but kept if they're within the kept range.
 *
 * @param keepRecentCount Number of recent user/assistant exchanges to keep (default: 4)
 */
class DefaultHistoryCompactor(
    private val keepRecentCount: Int = 4
) : HistoryCompactor {

    override suspend fun compact(
        messages: List<ChatMessage>,
        fromMode: AssistantMode,
        toMode: AssistantMode
    ): CompactedHistory {
        if (messages.isEmpty()) {
            return CompactedHistory(emptyList(), null)
        }

        // Filter out tool messages for counting, but we'll include them if within kept range
        val nonToolMessages = messages.filter { it.role != MessageRole.TOOL }

        if (nonToolMessages.size <= keepRecentCount) {
            // Keep all messages as-is
            return CompactedHistory(messages, null)
        }

        // Keep the last N non-tool messages
        val keptNonToolMessages = nonToolMessages.takeLast(keepRecentCount)
        val earliestKeptTimestamp = keptNonToolMessages.firstOrNull()?.timestamp ?: 0L

        // Keep all messages from the earliest kept message onwards
        val keptMessages = messages.filter { it.timestamp >= earliestKeptTimestamp }

        // Create a summary of what was discussed before
        val oldMessages = messages.filter { it.timestamp < earliestKeptTimestamp }
        val contextSummary = if (oldMessages.isNotEmpty()) {
            buildContextSummary(oldMessages, fromMode)
        } else {
            null
        }

        return CompactedHistory(keptMessages, contextSummary)
    }

    private fun buildContextSummary(oldMessages: List<ChatMessage>, fromMode: AssistantMode): String {
        // Build a simple summary of what topics were discussed
        val userMessages = oldMessages.filter { it.role == MessageRole.USER }
        val topicCount = userMessages.size

        return buildString {
            appendLine("[Previous context from ${fromMode.name} mode - $topicCount earlier exchanges]")
            if (userMessages.isNotEmpty()) {
                appendLine("Earlier topics discussed:")
                userMessages.takeLast(3).forEach { msg ->
                    val preview = msg.content.take(100).replace("\n", " ")
                    val suffix = if (msg.content.length > 100) "..." else ""
                    appendLine("- $preview$suffix")
                }
            }
        }.trimEnd()
    }
}

/**
 * History compactor that uses the LLM to create a summary.
 * This provides better context but has latency and cost implications.
 *
 * @param keepRecentCount Number of recent exchanges to keep verbatim
 * @param summarizer Function to summarize old messages using the LLM
 */
class LlmHistoryCompactor(
    private val keepRecentCount: Int = 4,
    private val summarizer: suspend (messages: List<ChatMessage>, fromMode: AssistantMode, toMode: AssistantMode) -> String?
) : HistoryCompactor {

    override suspend fun compact(
        messages: List<ChatMessage>,
        fromMode: AssistantMode,
        toMode: AssistantMode
    ): CompactedHistory {
        if (messages.isEmpty()) {
            return CompactedHistory(emptyList(), null)
        }

        val nonToolMessages = messages.filter { it.role != MessageRole.TOOL }

        if (nonToolMessages.size <= keepRecentCount) {
            return CompactedHistory(messages, null)
        }

        val keptNonToolMessages = nonToolMessages.takeLast(keepRecentCount)
        val earliestKeptTimestamp = keptNonToolMessages.firstOrNull()?.timestamp ?: 0L

        val keptMessages = messages.filter { it.timestamp >= earliestKeptTimestamp }
        val oldMessages = messages.filter { it.timestamp < earliestKeptTimestamp }

        val contextSummary = if (oldMessages.isNotEmpty()) {
            summarizer(oldMessages, fromMode, toMode)
        } else {
            null
        }

        return CompactedHistory(keptMessages, contextSummary)
    }
}

/**
 * History compactor that clears all history on mode switch.
 * Use this for modes that should start completely fresh.
 */
class ClearingHistoryCompactor : HistoryCompactor {
    override suspend fun compact(
        messages: List<ChatMessage>,
        fromMode: AssistantMode,
        toMode: AssistantMode
    ): CompactedHistory {
        return CompactedHistory(
            keptMessages = emptyList(),
            contextSummary = "Switched from ${fromMode.name} mode."
        )
    }
}
