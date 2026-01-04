package com.lelloman.simpleaiassistant.model

sealed class StreamEvent {
    data class Text(val content: String) : StreamEvent()
    data class ToolUse(val id: String, val name: String, val input: Map<String, Any?>) : StreamEvent()
    data class Error(val message: String) : StreamEvent()
    data object Done : StreamEvent()
}
