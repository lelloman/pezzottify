package com.lelloman.simpleaiassistant.llm

import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.flow.Flow

interface LlmProvider {
    val id: String
    val displayName: String

    fun streamChat(
        messages: List<ChatMessage>,
        tools: List<ToolSpec>,
        systemPrompt: String
    ): Flow<StreamEvent>

    suspend fun testConnection(): Result<Unit>

    suspend fun listModels(): Result<List<String>>

    /**
     * Detect the language of the given text.
     * Returns ISO 639-1 language code (e.g., "en", "es", "ja") or null if detection not supported.
     *
     * Default implementation returns null, meaning the caller should fall back to
     * asking the main LLM for detection.
     */
    suspend fun detectLanguage(text: String): String? = null
}
