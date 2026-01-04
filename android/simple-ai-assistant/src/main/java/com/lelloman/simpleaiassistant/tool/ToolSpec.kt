package com.lelloman.simpleaiassistant.tool

/**
 * Specification of a tool that can be used by the LLM.
 * This is the metadata sent to the LLM so it knows what tools are available.
 */
data class ToolSpec(
    val name: String,
    val description: String,
    val inputSchema: Map<String, Any?>
)
