package com.lelloman.simpleaiassistant.tool

/**
 * A tool that can be executed by the AI assistant.
 * Host apps implement this interface to provide app-specific functionality.
 */
interface Tool {
    /**
     * The specification of this tool (name, description, input schema).
     * This is sent to the LLM so it knows how to use the tool.
     */
    val spec: ToolSpec

    /**
     * Execute the tool with the given input.
     * @param input The input parameters from the LLM's tool call.
     * @return The result of the tool execution.
     */
    suspend fun execute(input: Map<String, Any?>): ToolResult
}
