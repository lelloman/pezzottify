package com.lelloman.simpleaiassistant.tool

data class ToolResult(
    val success: Boolean,
    val data: Any? = null,
    val error: String? = null
)
