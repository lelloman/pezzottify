package com.lelloman.pezzottify.android.domain.catalogsync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Payload structure for catalog_invalidation WebSocket messages.
 *
 * Matches the server's CatalogInvalidationMessage.
 */
@Serializable
private data class CatalogInvalidationPayload(
    val seq: Long,
    val event_type: String,
    val content_type: String,
    val content_id: String,
    val timestamp: Long,
)

/**
 * Handler for catalog invalidation messages from WebSocket.
 *
 * Registers with WebSocketManager to handle "catalog_invalidation" type messages
 * and dispatches events to CatalogSyncManager for processing.
 */
@Singleton
class CatalogWebSocketHandler @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val catalogSyncManager: CatalogSyncManager,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(CatalogWebSocketHandler::class)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val json = Json {
        ignoreUnknownKeys = true
    }

    private val handler = MessageHandler { type, payload ->
        logger.debug("Received catalog_invalidation message: type=$type")
        if (payload != null) {
            handleCatalogInvalidation(payload)
        } else {
            logger.warn("catalog_invalidation message has no payload")
        }
    }

    override fun initialize() {
        logger.info("Registering catalog_invalidation message handler")
        webSocketManager.registerHandler(PREFIX, handler)
    }

    private fun handleCatalogInvalidation(payloadString: String) {
        try {
            val payload = json.decodeFromString<CatalogInvalidationPayload>(payloadString)

            // Convert string event_type and content_type to enums
            val eventType = parseEventType(payload.event_type)
            val contentType = parseContentType(payload.content_type)

            if (eventType == null) {
                logger.warn("Unknown catalog event type: ${payload.event_type}")
                return
            }

            if (contentType == null) {
                logger.warn("Unknown catalog content type: ${payload.content_type}")
                return
            }

            val event = CatalogEvent(
                seq = payload.seq,
                eventType = eventType,
                contentType = contentType,
                contentId = payload.content_id,
                timestamp = payload.timestamp,
                triggeredBy = null,
            )

            scope.launch {
                catalogSyncManager.handleRealtimeEvent(event)
            }
        } catch (e: Exception) {
            logger.error("Failed to parse catalog_invalidation payload: $payloadString", e)
        }
    }

    private fun parseEventType(type: String): CatalogEventType? = when (type) {
        "album_updated" -> CatalogEventType.AlbumUpdated
        "artist_updated" -> CatalogEventType.ArtistUpdated
        "track_updated" -> CatalogEventType.TrackUpdated
        "album_added" -> CatalogEventType.AlbumAdded
        "artist_added" -> CatalogEventType.ArtistAdded
        "track_added" -> CatalogEventType.TrackAdded
        else -> null
    }

    private fun parseContentType(type: String): CatalogContentType? = when (type) {
        "album" -> CatalogContentType.Album
        "artist" -> CatalogContentType.Artist
        "track" -> CatalogContentType.Track
        else -> null
    }

    companion object {
        private const val PREFIX = "catalog_invalidation"
    }
}
