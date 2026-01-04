package com.lelloman.simpleaiassistant.provider.ollama

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/**
 * Ollama API request/response models.
 * Based on https://github.com/ollama/ollama/blob/main/docs/api.md
 */

@Serializable
data class OllamaChatRequest(
    val model: String,
    val messages: List<OllamaMessage>,
    val stream: Boolean = true,
    val tools: List<OllamaTool>? = null
)

@Serializable
data class OllamaMessage(
    val role: String,
    val content: String,
    @SerialName("tool_calls") val toolCalls: List<OllamaToolCall>? = null
)

@Serializable
data class OllamaTool(
    val type: String = "function",
    val function: OllamaFunction
)

@Serializable
data class OllamaFunction(
    val name: String,
    val description: String,
    val parameters: JsonElement
)

@Serializable
data class OllamaToolCall(
    val function: OllamaToolCallFunction
)

@Serializable
data class OllamaToolCallFunction(
    val name: String,
    val arguments: JsonObject
)

@Serializable
data class OllamaChatResponse(
    val model: String? = null,
    val message: OllamaMessage? = null,
    val done: Boolean = false,
    @SerialName("done_reason") val doneReason: String? = null,
    val error: String? = null
)

@Serializable
data class OllamaTagsResponse(
    val models: List<OllamaModel>
)

@Serializable
data class OllamaModel(
    val name: String,
    val model: String? = null,
    val size: Long? = null
)
