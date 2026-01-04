package com.lelloman.simpleaiassistant.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.ToolCall
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

@Entity(tableName = "chat_messages")
data class ChatMessageEntity(
    @PrimaryKey
    val id: String,
    val role: String,
    val content: String,
    val toolCallsJson: String?,
    val toolCallId: String?,
    val toolName: String?,
    val timestamp: Long
) {
    fun toDomain(): ChatMessage = ChatMessage(
        id = id,
        role = MessageRole.valueOf(role),
        content = content,
        toolCalls = toolCallsJson?.let { json.decodeFromString<List<ToolCallSerializable>>(it) }
            ?.map { it.toDomain() },
        toolCallId = toolCallId,
        toolName = toolName,
        timestamp = timestamp
    )

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        fun fromDomain(message: ChatMessage): ChatMessageEntity = ChatMessageEntity(
            id = message.id,
            role = message.role.name,
            content = message.content,
            toolCallsJson = message.toolCalls?.let { calls ->
                json.encodeToString(calls.map { ToolCallSerializable.fromDomain(it) })
            },
            toolCallId = message.toolCallId,
            toolName = message.toolName,
            timestamp = message.timestamp
        )
    }
}

@kotlinx.serialization.Serializable
private data class ToolCallSerializable(
    val id: String,
    val name: String,
    val input: Map<String, kotlinx.serialization.json.JsonElement>
) {
    fun toDomain(): ToolCall = ToolCall(
        id = id,
        name = name,
        input = input.mapValues { jsonElementToAny(it.value) }
    )

    companion object {
        fun fromDomain(toolCall: ToolCall): ToolCallSerializable = ToolCallSerializable(
            id = toolCall.id,
            name = toolCall.name,
            input = toolCall.input.mapValues { anyToJsonElement(it.value) }
        )

        private fun jsonElementToAny(element: kotlinx.serialization.json.JsonElement): Any? {
            return when (element) {
                is kotlinx.serialization.json.JsonNull -> null
                is kotlinx.serialization.json.JsonPrimitive -> {
                    if (element.isString) {
                        element.content
                    } else {
                        element.content.toBooleanStrictOrNull()
                            ?: element.content.toLongOrNull()
                            ?: element.content.toDoubleOrNull()
                            ?: element.content
                    }
                }
                is kotlinx.serialization.json.JsonArray -> element.map { jsonElementToAny(it) }
                is kotlinx.serialization.json.JsonObject -> element.mapValues { jsonElementToAny(it.value) }
            }
        }

        private fun anyToJsonElement(value: Any?): kotlinx.serialization.json.JsonElement {
            return when (value) {
                null -> kotlinx.serialization.json.JsonNull
                is Boolean -> kotlinx.serialization.json.JsonPrimitive(value)
                is Number -> kotlinx.serialization.json.JsonPrimitive(value)
                is String -> kotlinx.serialization.json.JsonPrimitive(value)
                is List<*> -> kotlinx.serialization.json.JsonArray(value.map { anyToJsonElement(it) })
                is Map<*, *> -> kotlinx.serialization.json.JsonObject(
                    value.entries.associate { (k, v) -> k.toString() to anyToJsonElement(v) }
                )
                else -> kotlinx.serialization.json.JsonPrimitive(value.toString())
            }
        }
    }
}
