package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.skeleton.CatalogSkeletonSyncer
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
 * Payload structure for catalog_updated WebSocket messages.
 */
@Serializable
private data class CatalogUpdatedPayload(
    val skeleton_version: Long,
)

/**
 * Handler for catalog update messages from WebSocket.
 *
 * Registers with WebSocketManager to handle "catalog_updated" type messages
 * and triggers skeleton sync when the catalog has changed on the server.
 */
@Singleton
class CatalogWebSocketHandler @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val skeletonSyncer: CatalogSkeletonSyncer,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(CatalogWebSocketHandler::class)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val json = Json {
        ignoreUnknownKeys = true
    }

    private val handler = MessageHandler { type, payload ->
        logger.debug("Received catalog_updated message: type=$type")
        handleCatalogUpdated(payload)
    }

    override fun initialize() {
        logger.info("Registering catalog_updated message handler")
        webSocketManager.registerHandler(MESSAGE_TYPE, handler)
    }

    private fun handleCatalogUpdated(payloadString: String?) {
        // Parse payload to get skeleton version (optional, mainly for logging)
        val skeletonVersion = payloadString?.let {
            try {
                json.decodeFromString<CatalogUpdatedPayload>(it).skeleton_version
            } catch (e: Exception) {
                logger.warn("Failed to parse catalog_updated payload: $it")
                null
            }
        }

        logger.info("Catalog updated on server (version=$skeletonVersion), triggering skeleton sync")

        scope.launch {
            when (val result = skeletonSyncer.sync()) {
                is CatalogSkeletonSyncer.SyncResult.Success ->
                    logger.info("Skeleton sync completed after catalog_updated")
                is CatalogSkeletonSyncer.SyncResult.AlreadyUpToDate ->
                    logger.info("Skeleton already up to date after catalog_updated")
                is CatalogSkeletonSyncer.SyncResult.Failed ->
                    logger.error("Skeleton sync failed after catalog_updated: ${result.error}")
            }
        }
    }

    companion object {
        private const val MESSAGE_TYPE = "catalog_updated"
    }
}
