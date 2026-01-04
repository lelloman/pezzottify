package com.lelloman.simpleaiassistant.model

data class ToolCall(
    val id: String,
    val name: String,
    val input: Map<String, Any?>
)
