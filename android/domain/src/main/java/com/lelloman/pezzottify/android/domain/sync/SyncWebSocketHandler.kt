package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Payload structure for sync WebSocket messages.
 */
@Serializable
private data class SyncMessagePayload(
    val event: StoredEvent,
)

/**
 * Handler for sync messages from WebSocket.
 *
 * Registers with WebSocketManager to handle "sync" type messages
 * and dispatches events to SyncManager for processing.
 */
@Singleton
class SyncWebSocketHandler @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val syncManager: SyncManager,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(SyncWebSocketHandler::class)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val json = Json {
        ignoreUnknownKeys = true
    }

    private val handler = MessageHandler { type, payload ->
        logger.debug("Received sync message: type=$type")
        if (payload != null) {
            handleSyncPayload(payload)
        } else {
            logger.warn("Sync message has no payload")
        }
    }

    override fun initialize() {
        logger.info("Registering sync message handler")
        webSocketManager.registerHandler(PREFIX, handler)
    }

    private fun handleSyncPayload(payloadString: String) {
        try {
            val payload = json.decodeFromString<SyncMessagePayload>(payloadString)
            scope.launch {
                syncManager.handleSyncMessage(payload.event)
            }
        } catch (e: Exception) {
            logger.error("Failed to parse sync message payload: $payloadString", e)
        }
    }

    companion object {
        private const val PREFIX = "sync"
    }
}
