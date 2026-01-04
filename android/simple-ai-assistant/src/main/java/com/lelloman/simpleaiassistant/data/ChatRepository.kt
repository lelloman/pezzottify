package com.lelloman.simpleaiassistant.data

import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

/**
 * Repository for chat operations.
 * Handles message persistence, LLM communication, and tool execution.
 */
interface ChatRepository {

    /**
     * All messages in the current conversation.
     */
    val messages: Flow<List<ChatMessage>>

    /**
     * Current streaming text from the assistant (while response is being generated).
     */
    val streamingText: StateFlow<String>

    /**
     * Whether the assistant is currently generating a response.
     */
    val isStreaming: StateFlow<Boolean>

    /**
     * Current language preference.
     */
    val language: StateFlow<Language?>

    /**
     * Whether the language is currently being auto-detected.
     */
    val isDetectingLanguage: StateFlow<Boolean>

    /**
     * Send a message and get a response from the assistant.
     * This will:
     * 1. Save the user message
     * 2. Stream the response from the LLM
     * 3. Execute any tool calls
     * 4. Save the assistant response
     */
    suspend fun sendMessage(text: String)

    /**
     * Set the language preference.
     */
    suspend fun setLanguage(language: Language?)

    /**
     * Clear all messages.
     */
    suspend fun clearHistory()

    /**
     * Restart the conversation from a specific message.
     * Deletes all messages after the given message and re-sends it to get a new response.
     *
     * @param messageId The ID of the user message to restart from
     */
    suspend fun restartFromMessage(messageId: String)
}
