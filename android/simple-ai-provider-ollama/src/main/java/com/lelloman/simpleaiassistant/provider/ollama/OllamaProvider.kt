package com.lelloman.simpleaiassistant.provider.ollama

import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.BufferedReader
import java.util.UUID
import java.util.concurrent.TimeUnit

class OllamaProvider(
    private val config: OllamaConfig,
    private val httpClient: OkHttpClient = createDefaultClient(config)
) : LlmProvider {

    override val id: String = "ollama"
    override val displayName: String = "Ollama"

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    override fun streamChat(
        messages: List<ChatMessage>,
        tools: List<ToolSpec>,
        systemPrompt: String
    ): Flow<StreamEvent> = flow {
        val ollamaMessages = buildMessageList(messages, systemPrompt)
        val ollamaTools = if (tools.isNotEmpty()) {
            tools.map { it.toOllamaTool() }
        } else null

        val request = OllamaChatRequest(
            model = config.model,
            messages = ollamaMessages,
            stream = true,
            tools = ollamaTools
        )

        val requestBody = json.encodeToString(request)
            .toRequestBody("application/json".toMediaType())

        val httpRequest = Request.Builder()
            .url("${config.baseUrl}/api/chat")
            .post(requestBody)
            .build()

        try {
            val response = httpClient.newCall(httpRequest).execute()

            if (!response.isSuccessful) {
                emit(StreamEvent.Error("HTTP ${response.code}: ${response.message}"))
                return@flow
            }

            val reader = response.body?.charStream()?.buffered()
            if (reader == null) {
                emit(StreamEvent.Error("Empty response body"))
                return@flow
            }

            processStreamResponse(reader).collect { event ->
                emit(event)
            }
        } catch (e: Exception) {
            emit(StreamEvent.Error(e.message ?: "Unknown error"))
        }
    }.flowOn(Dispatchers.IO)

    private fun processStreamResponse(reader: BufferedReader): Flow<StreamEvent> = flow {
        reader.useLines { lines ->
            for (line in lines) {
                if (line.isBlank()) continue

                val parsed = try {
                    json.decodeFromString<OllamaChatResponse>(line)
                } catch (e: Exception) {
                    emit(StreamEvent.Error("Failed to parse response: ${e.message}"))
                    continue
                }

                if (parsed.error != null) {
                    emit(StreamEvent.Error(parsed.error))
                    return@useLines
                }

                val message = parsed.message
                if (message != null) {
                    // Emit text content
                    if (message.content.isNotEmpty()) {
                        emit(StreamEvent.Text(message.content))
                    }

                    // Emit tool calls
                    message.toolCalls?.forEach { toolCall ->
                        emit(
                            StreamEvent.ToolUse(
                                id = UUID.randomUUID().toString(),
                                name = toolCall.function.name,
                                input = toolCall.function.arguments.toMap()
                            )
                        )
                    }
                }

                if (parsed.done) {
                    emit(StreamEvent.Done)
                    return@useLines
                }
            }
        }
    }

    private fun JsonObject.toMap(): Map<String, Any?> {
        return entries.associate { (key, value) ->
            key to value.toKotlinValue()
        }
    }

    private fun JsonElement.toKotlinValue(): Any? {
        return when (this) {
            is JsonPrimitive -> {
                when {
                    isString -> content
                    content == "true" -> true
                    content == "false" -> false
                    content == "null" -> null
                    content.contains('.') -> content.toDoubleOrNull()
                    else -> content.toLongOrNull() ?: content
                }
            }
            is JsonObject -> toMap()
            is kotlinx.serialization.json.JsonArray -> map { it.toKotlinValue() }
            is kotlinx.serialization.json.JsonNull -> null
        }
    }

    override suspend fun testConnection(): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url("${config.baseUrl}/api/tags")
                .get()
                .build()

            val response = httpClient.newCall(request).execute()
            if (response.isSuccessful) {
                Result.success(Unit)
            } else {
                Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    override suspend fun listModels(): Result<List<String>> = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url("${config.baseUrl}/api/tags")
                .get()
                .build()

            val response = httpClient.newCall(request).execute()
            if (!response.isSuccessful) {
                return@withContext Result.failure(
                    Exception("HTTP ${response.code}: ${response.message}")
                )
            }

            val body = response.body?.string()
            if (body == null) {
                return@withContext Result.failure(Exception("Empty response"))
            }

            val tagsResponse = json.decodeFromString<OllamaTagsResponse>(body)
            Result.success(tagsResponse.models.map { it.name })
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    override suspend fun detectLanguage(text: String): String? = withContext(Dispatchers.IO) {
        try {
            val prompt = """Detect the language of the following text and respond with ONLY the ISO 639-1 language code (e.g., "en", "es", "fr", "de", "ja", "zh"). Do not include any other text or explanation.

Text: "$text"

Language code:"""

            val request = OllamaChatRequest(
                model = config.model,
                messages = listOf(OllamaMessage(role = "user", content = prompt)),
                stream = false,
                tools = null
            )

            val requestBody = json.encodeToString(request)
                .toRequestBody("application/json".toMediaType())

            val httpRequest = Request.Builder()
                .url("${config.baseUrl}/api/chat")
                .post(requestBody)
                .build()

            val response = httpClient.newCall(httpRequest).execute()
            if (!response.isSuccessful) {
                return@withContext null
            }

            val body = response.body?.string() ?: return@withContext null
            val chatResponse = json.decodeFromString<OllamaChatResponse>(body)

            // Extract and clean the language code
            chatResponse.message?.content?.trim()?.lowercase()?.take(2)
        } catch (e: Exception) {
            null
        }
    }

    private fun buildMessageList(
        messages: List<ChatMessage>,
        systemPrompt: String
    ): List<OllamaMessage> {
        val result = mutableListOf<OllamaMessage>()

        // Add system prompt as first message
        if (systemPrompt.isNotEmpty()) {
            result.add(OllamaMessage(role = "system", content = systemPrompt))
        }

        // Convert chat messages
        for (message in messages) {
            val role = when (message.role) {
                MessageRole.USER -> "user"
                MessageRole.ASSISTANT -> "assistant"
                MessageRole.TOOL -> "tool"
            }
            result.add(OllamaMessage(role = role, content = message.content))
        }

        return result
    }

    private fun ToolSpec.toOllamaTool(): OllamaTool {
        return OllamaTool(
            type = "function",
            function = OllamaFunction(
                name = name,
                description = description,
                parameters = inputSchemaToJsonElement()
            )
        )
    }

    private fun ToolSpec.inputSchemaToJsonElement(): JsonElement {
        return mapToJsonElement(inputSchema)
    }

    private fun mapToJsonElement(map: Map<String, Any?>): JsonElement {
        return buildJsonObject {
            for ((key, value) in map) {
                when (value) {
                    is String -> put(key, value)
                    is Number -> put(key, value)
                    is Boolean -> put(key, value)
                    is Map<*, *> -> {
                        @Suppress("UNCHECKED_CAST")
                        put(key, mapToJsonElement(value as Map<String, Any?>))
                    }
                    is List<*> -> {
                        put(key, kotlinx.serialization.json.buildJsonArray {
                            for (item in value) {
                                when (item) {
                                    is String -> add(JsonPrimitive(item))
                                    is Number -> add(JsonPrimitive(item))
                                    is Boolean -> add(JsonPrimitive(item))
                                    is Map<*, *> -> {
                                        @Suppress("UNCHECKED_CAST")
                                        add(mapToJsonElement(item as Map<String, Any?>))
                                    }
                                    null -> add(kotlinx.serialization.json.JsonNull)
                                    else -> add(JsonPrimitive(item.toString()))
                                }
                            }
                        })
                    }
                    null -> put(key, kotlinx.serialization.json.JsonNull)
                    else -> put(key, value.toString())
                }
            }
        }
    }

    companion object {
        private fun createDefaultClient(config: OllamaConfig): OkHttpClient {
            return OkHttpClient.Builder()
                .connectTimeout(config.timeoutMs, TimeUnit.MILLISECONDS)
                .readTimeout(config.timeoutMs, TimeUnit.MILLISECONDS)
                .writeTimeout(config.timeoutMs, TimeUnit.MILLISECONDS)
                .build()
        }
    }
}
