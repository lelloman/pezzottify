package com.lelloman.simpleaiassistant.ui

import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language

data class ChatUiState(
    val messages: List<ChatMessage> = emptyList(),
    val streamingText: String = "",
    val isStreaming: Boolean = false,
    val language: Language? = null,
    val debugMode: Boolean = false,
    val error: String? = null
)
