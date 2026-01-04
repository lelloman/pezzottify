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
}
