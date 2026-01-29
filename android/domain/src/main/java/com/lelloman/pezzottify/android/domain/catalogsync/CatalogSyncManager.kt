package com.lelloman.pezzottify.android.domain.catalogsync

import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages catalog sync operations.
 *
 * Handles both real-time events from WebSocket and catch-up via REST API.
 * When catalog content changes on the server, this manager invalidates
 * the corresponding cached data so that fresh data is fetched on next access.
 */
@Singleton
class CatalogSyncManager @Inject constructor(
    private val catalogSyncStore: CatalogSyncStore,
    private val staticsStore: StaticsStore,
    private val staticsCache: StaticsCache,
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) {
    private val logger: Logger by loggerFactory

    /**
     * Catch up on missed catalog events.
     *
     * Called when WebSocket reconnects or when a gap in sequence numbers is detected.
     * Fetches all events since the last processed sequence number from the server.
     */
    suspend fun catchUp() {
        val currentSeq = catalogSyncStore.currentSeq.value
        logger.info("Catching up on catalog events since seq=$currentSeq")

        when (val response = remoteApiClient.getCatalogSync(since = currentSeq)) {
            is RemoteApiResponse.Success -> {
                val data = response.data
                logger.info("Received ${data.events.size} catalog events (current_seq=${data.currentSeq})")

                // Apply each event
                data.events.forEach { event ->
                    applyEvent(event)
                }

                // Update cursor to server's current sequence
                catalogSyncStore.setCurrentSeq(data.currentSeq)
            }
            is RemoteApiResponse.Error.Network -> {
                logger.warn("Network error during catalog catch-up")
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("Unauthorized during catalog catch-up")
            }
            is RemoteApiResponse.Error.NotFound -> {
                logger.warn("Catalog sync endpoint not found")
            }
            is RemoteApiResponse.Error.EventsPruned -> {
                // Old events were pruned - reset cursor to 0 so we don't miss future events
                logger.warn("Catalog events pruned, resetting cursor")
                catalogSyncStore.setCurrentSeq(0)
            }
            is RemoteApiResponse.Error.Unknown -> {
                logger.error("Unknown error during catalog catch-up: ${response.message}")
            }
        }
    }

    /**
     * Handle a real-time catalog event from WebSocket.
     *
     * If the event sequence is not consecutive (gap detected), triggers
     * a full catch-up via REST API.
     */
    suspend fun handleRealtimeEvent(event: CatalogEvent) {
        val expectedSeq = catalogSyncStore.currentSeq.value + 1

        if (event.seq != expectedSeq) {
            // Gap detected - there were events we missed
            logger.warn("Catalog event sequence gap detected: expected=$expectedSeq, received=${event.seq}")
            catchUp()
            return
        }

        // Apply the event
        applyEvent(event)

        // Update cursor
        catalogSyncStore.setCurrentSeq(event.seq)
    }

    /**
     * Apply a single catalog event by invalidating cached data.
     */
    private suspend fun applyEvent(event: CatalogEvent) {
        logger.debug("Applying catalog event: ${event.eventType} ${event.contentType} ${event.contentId}")

        when (event.contentType) {
            CatalogContentType.Album -> {
                // Invalidate in-memory cache
                staticsCache.albumCache.remove(event.contentId)
                // Invalidate persistent store
                staticsStore.deleteAlbum(event.contentId)
                logger.info("Invalidated album: ${event.contentId}")
            }
            CatalogContentType.Artist -> {
                staticsCache.artistCache.remove(event.contentId)
                staticsStore.deleteArtist(event.contentId)
                logger.info("Invalidated artist: ${event.contentId}")
            }
            CatalogContentType.Track -> {
                staticsCache.trackCache.remove(event.contentId)
                staticsStore.deleteTrack(event.contentId)
                logger.info("Invalidated track: ${event.contentId}")
            }
        }
    }

    /**
     * Reset sync state (e.g., on logout).
     */
    suspend fun reset() {
        logger.info("Resetting catalog sync state")
        catalogSyncStore.clear()
    }
}
