package com.lelloman.pezzottify.android.domain.websocket

import kotlinx.coroutines.flow.StateFlow

/**
 * Connection state for WebSocket.
 */
sealed interface ConnectionState {
    data object Disconnected : ConnectionState
    data object Connecting : ConnectionState
    data class Connected(val deviceId: Int) : ConnectionState
    data class Error(val message: String) : ConnectionState
}

/**
 * Handler for incoming WebSocket messages.
 */
fun interface MessageHandler {
    fun onMessage(type: String, payload: String?)
}

/**
 * Manager for WebSocket connection to the server.
 */
interface WebSocketManager {

    /**
     * Current connection state.
     */
    val connectionState: StateFlow<ConnectionState>

    /**
     * Establish WebSocket connection to the server.
     */
    suspend fun connect()

    /**
     * Close WebSocket connection.
     */
    suspend fun disconnect()

    /**
     * Send a message to the server.
     *
     * @param type Message type (e.g., "ping")
     * @param payload Optional JSON payload
     */
    fun send(type: String, payload: Any? = null)

    /**
     * Register a handler for messages with a given prefix.
     *
     * @param prefix Message type prefix (e.g., "sync" to handle "sync.liked", "sync.playlist", etc.)
     * @param handler Handler to call when a matching message is received
     */
    fun registerHandler(prefix: String, handler: MessageHandler)

    /**
     * Unregister a previously registered handler.
     *
     * @param prefix The prefix that was used to register the handler
     */
    fun unregisterHandler(prefix: String)
}
