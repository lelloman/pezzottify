package com.lelloman.pezzottify.android.domain.websocket

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/**
 * Message sent from client to server.
 */
@Serializable
data class ClientMessage(
    val type: String,
    val payload: JsonElement? = null,
)

/**
 * Message received from server.
 */
@Serializable
data class ServerMessage(
    val type: String,
    val payload: JsonElement? = null,
)

/**
 * Payload of the "connected" message from server.
 */
@Serializable
data class ConnectedPayload(
    val device_id: Int,
)

/**
 * Payload of the "error" message from server.
 */
@Serializable
data class ErrorPayload(
    val code: String,
    val message: String,
)
