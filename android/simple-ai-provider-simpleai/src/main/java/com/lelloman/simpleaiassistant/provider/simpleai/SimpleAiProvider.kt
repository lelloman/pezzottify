package com.lelloman.simpleaiassistant.provider.simpleai

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import com.lelloman.simpleai.ISimpleAI
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * LLM Provider that communicates with the SimpleAI Android app via AIDL.
 *
 * SimpleAI app handles the actual cloud LLM communication, this provider
 * just binds to its service and forwards requests.
 */
class SimpleAiProvider(
    private val context: Context,
    private val config: SimpleAiConfig
) : LlmProvider {

    companion object {
        private const val TAG = "SimpleAiProvider"
        private const val PROTOCOL_VERSION = 1
        private const val PACKAGE = "com.lelloman.simpleai"
        private const val SERVICE_ACTION = "com.lelloman.simpleai.SIMPLE_AI_SERVICE"
    }

    override val id: String = "simpleai"
    override val displayName: String = "SimpleAI"

    private val json = Json { ignoreUnknownKeys = true }

    private var simpleAi: ISimpleAI? = null
    private var isBound = false
    private var serviceConnection: ServiceConnection? = null

    override fun streamChat(
        messages: List<ChatMessage>,
        tools: List<ToolSpec>,
        systemPrompt: String
    ): Flow<StreamEvent> = flow {
        try {
            val service = ensureConnected()

            val messagesJson = buildMessagesJson(messages)
            val toolsJson = if (tools.isNotEmpty()) buildToolsJson(tools) else null

            val responseJson = service.cloudChat(
                PROTOCOL_VERSION,
                messagesJson,
                toolsJson,
                systemPrompt,
                config.authTokenProvider()  // Fetch fresh token for each request
            )

            Log.d(TAG, "cloudChat response: $responseJson")

            val response = parseCloudChatResponse(responseJson)
            response.forEach { event -> emit(event) }

        } catch (e: Exception) {
            Log.e(TAG, "streamChat failed", e)
            emit(StreamEvent.Error(e.message ?: "Unknown error"))
        }
    }.flowOn(Dispatchers.IO)

    private fun parseCloudChatResponse(responseJson: String): List<StreamEvent> {
        val events = mutableListOf<StreamEvent>()

        try {
            val response = json.parseToJsonElement(responseJson).jsonObject
            val status = response["status"]?.jsonPrimitive?.contentOrNull

            if (status == "error") {
                val error = response["error"]?.jsonObject
                val message = error?.get("message")?.jsonPrimitive?.contentOrNull ?: "Unknown error"
                events.add(StreamEvent.Error(message))
                return events
            }

            val data = response["data"]?.jsonObject
            if (data == null) {
                events.add(StreamEvent.Error("Invalid response: missing data"))
                return events
            }

            // Extract content
            val content = data["content"]?.jsonPrimitive?.contentOrNull
            if (!content.isNullOrEmpty()) {
                events.add(StreamEvent.Text(content))
            }

            // Extract tool calls
            val toolCalls = data["toolCalls"]?.jsonArray
            toolCalls?.forEach { toolCallElement ->
                val toolCall = toolCallElement.jsonObject
                val id = toolCall["id"]?.jsonPrimitive?.contentOrNull ?: ""

                // Handle OpenAI-style format: { "function": { "name": "...", "arguments": "..." } }
                val functionObj = toolCall["function"]?.jsonObject
                val name = functionObj?.get("name")?.jsonPrimitive?.contentOrNull
                    ?: toolCall["name"]?.jsonPrimitive?.contentOrNull
                    ?: ""

                val arguments: Map<String, Any?> = when {
                    functionObj != null -> {
                        val argsStr = functionObj["arguments"]?.jsonPrimitive?.contentOrNull
                        if (argsStr != null) {
                            try {
                                json.parseToJsonElement(argsStr).jsonObject.toMap()
                            } catch (_: Exception) {
                                emptyMap()
                            }
                        } else {
                            emptyMap()
                        }
                    }
                    else -> toolCall["arguments"]?.jsonObject?.toMap() ?: emptyMap()
                }

                events.add(StreamEvent.ToolUse(id = id, name = name, input = arguments))
            }

            events.add(StreamEvent.Done)

        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse response: $responseJson", e)
            events.add(StreamEvent.Error("Failed to parse response: ${e.message}"))
        }

        return events
    }

    private fun JsonObject.toMap(): Map<String, Any?> {
        return entries.associate { (key, value) ->
            key to when (value) {
                is kotlinx.serialization.json.JsonPrimitive -> {
                    when {
                        value.isString -> value.content
                        value.content == "true" -> true
                        value.content == "false" -> false
                        value.content == "null" -> null
                        value.content.contains('.') -> value.content.toDoubleOrNull()
                        else -> value.content.toLongOrNull() ?: value.content
                    }
                }
                is JsonObject -> value.toMap()
                is kotlinx.serialization.json.JsonArray -> value.map {
                    when (it) {
                        is kotlinx.serialization.json.JsonPrimitive -> it.content
                        is JsonObject -> it.toMap()
                        else -> null
                    }
                }
                is kotlinx.serialization.json.JsonNull -> null
                else -> null
            }
        }
    }

    override suspend fun testConnection(): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            if (!isInstalled()) {
                return@withContext Result.failure(Exception("SimpleAI app is not installed"))
            }

            val service = ensureConnected()
            val infoJson = service.getServiceInfo(PROTOCOL_VERSION)
            val response = json.parseToJsonElement(infoJson).jsonObject
            val status = response["status"]?.jsonPrimitive?.contentOrNull

            if (status == "error") {
                val error = response["error"]?.jsonObject
                val message = error?.get("message")?.jsonPrimitive?.contentOrNull ?: "Unknown error"
                return@withContext Result.failure(Exception(message))
            }

            val data = response["data"]?.jsonObject
            val capabilities = data?.get("capabilities")?.jsonObject
            val cloudAiStatus = capabilities?.get("cloudAi")?.jsonObject
                ?.get("status")?.jsonPrimitive?.contentOrNull

            if (cloudAiStatus == "ready") {
                Result.success(Unit)
            } else {
                Result.failure(Exception("Cloud AI not ready: $cloudAiStatus"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    override suspend fun listModels(): Result<List<String>> = withContext(Dispatchers.IO) {
        // SimpleAI uses server-configured model, no model listing
        Result.success(emptyList())
    }

    override suspend fun detectLanguage(text: String): String? = withContext(Dispatchers.IO) {
        // SimpleAI app has translation capability but not direct language detection
        // Could potentially use translate with "auto" source, but for now return null
        null
    }

    private fun isInstalled(): Boolean {
        val intent = Intent(SERVICE_ACTION).apply {
            setPackage(PACKAGE)
        }
        return context.packageManager.resolveService(intent, 0) != null
    }

    private suspend fun ensureConnected(): ISimpleAI {
        simpleAi?.let { return it }

        return suspendCancellableCoroutine { continuation ->
            val intent = Intent(SERVICE_ACTION).apply {
                setPackage(PACKAGE)
            }

            val connection = object : ServiceConnection {
                override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
                    val svc = ISimpleAI.Stub.asInterface(service)
                    simpleAi = svc
                    isBound = true
                    serviceConnection = this
                    if (continuation.isActive) {
                        continuation.resume(svc)
                    }
                }

                override fun onServiceDisconnected(name: ComponentName?) {
                    simpleAi = null
                    isBound = false
                }
            }

            val bound = context.bindService(intent, connection, Context.BIND_AUTO_CREATE)
            if (!bound) {
                continuation.resumeWithException(
                    IllegalStateException("Failed to bind to SimpleAI service. Is SimpleAI installed?")
                )
            }

            continuation.invokeOnCancellation {
                if (isBound) {
                    try {
                        context.unbindService(connection)
                    } catch (_: Exception) {}
                }
            }
        }
    }

    private fun buildMessagesJson(messages: List<ChatMessage>): String {
        val messagesArray = buildJsonArray {
            for (message in messages) {
                add(buildJsonObject {
                    put("role", when (message.role) {
                        MessageRole.USER -> "user"
                        MessageRole.ASSISTANT -> "assistant"
                        MessageRole.TOOL -> "tool"
                    })
                    put("content", message.content)

                    message.toolCallId?.let { put("toolCallId", it) }

                    message.toolCalls?.let { calls ->
                        put("toolCalls", buildJsonArray {
                            for (call in calls) {
                                add(buildJsonObject {
                                    put("id", call.id)
                                    put("name", call.name)
                                    put("arguments", mapToJsonObject(call.input))
                                })
                            }
                        })
                    }
                })
            }
        }
        return messagesArray.toString()
    }

    private fun buildToolsJson(tools: List<ToolSpec>): String {
        val toolsArray = buildJsonArray {
            for (tool in tools) {
                add(buildJsonObject {
                    put("type", "function")
                    put("function", buildJsonObject {
                        put("name", tool.name)
                        put("description", tool.description)
                        put("parameters", mapToJsonObject(tool.inputSchema))
                    })
                })
            }
        }
        return toolsArray.toString()
    }

    private fun mapToJsonObject(map: Map<String, Any?>): JsonObject {
        return buildJsonObject {
            for ((key, value) in map) {
                when (value) {
                    is String -> put(key, value)
                    is Number -> put(key, value)
                    is Boolean -> put(key, value)
                    is Map<*, *> -> {
                        @Suppress("UNCHECKED_CAST")
                        put(key, mapToJsonObject(value as Map<String, Any?>))
                    }
                    is List<*> -> {
                        put(key, buildJsonArray {
                            for (item in value) {
                                when (item) {
                                    is String -> add(kotlinx.serialization.json.JsonPrimitive(item))
                                    is Number -> add(kotlinx.serialization.json.JsonPrimitive(item))
                                    is Boolean -> add(kotlinx.serialization.json.JsonPrimitive(item))
                                    is Map<*, *> -> {
                                        @Suppress("UNCHECKED_CAST")
                                        add(mapToJsonObject(item as Map<String, Any?>))
                                    }
                                    null -> add(kotlinx.serialization.json.JsonNull)
                                    else -> add(kotlinx.serialization.json.JsonPrimitive(item.toString()))
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

    /**
     * Release the service connection.
     */
    fun release() {
        serviceConnection?.let {
            try {
                context.unbindService(it)
            } catch (_: Exception) {}
        }
        simpleAi = null
        isBound = false
        serviceConnection = null
    }
}
