package com.lelloman.pezzottify.android.domain.listening

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/**
 * Background synchronizer that sends pending listening events to server.
 *
 * Extends BaseSynchronizer for consistent behavior with other synchronizers:
 * - Exponential backoff on failures
 * - Sleep/wake mechanism
 * - Session ID deduplication handles retries
 */
@Singleton
class ListeningEventSynchronizer @Inject constructor(
    private val listeningEventStore: ListeningEventStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<ListeningEvent>(
    logger = loggerFactory.getLogger(ListeningEventSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    override suspend fun getItemsToProcess(): List<ListeningEvent> =
        listeningEventStore.getPendingSyncEvents()

    override suspend fun processItem(item: ListeningEvent) {
        syncEvent(item)
    }

    override suspend fun onBeforeMainLoop() {
        // Cleanup old non-synced events on app startup
        val cutoff = timeProvider.nowUtcMs() - TimeUnit.DAYS.toMillis(CLEANUP_AGE_DAYS)
        val deleted = listeningEventStore.deleteOldNonSyncedEvents(cutoff)
        if (deleted > 0) {
            logger.info("Cleaned up $deleted old non-synced listening events")
        }
    }

    private suspend fun syncEvent(event: ListeningEvent) {
        // Player duration can still belong to the previous track at a transition.
        // Use catalog metadata, which is also the server's validation authority.
        val duration = staticsStore.getTrack(event.trackId).first()?.durationSeconds
        if (duration == null || duration <= 0) {
            logger.debug("Waiting for catalog duration before syncing ${event.sessionId}")
            return
        }
        listeningEventStore.updateSyncStatus(event.id, SyncStatus.Syncing)

        val result = remoteApiClient.recordListeningEvent(event.toSyncData().copy(
            trackDurationSeconds = duration,
            // Buffering/replays can inflate elapsed playing time beyond one track.
            // A listening session represents at most one complete track.
            durationSeconds = event.durationSeconds.coerceIn(0, duration),
        ))

        when (result) {
            is RemoteApiResponse.Success -> {
                // Mark as synced - deletion happens when session is finalized or new session starts
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.Synced)
                logger.debug("Successfully synced listening event ${event.sessionId}")
            }
            is RemoteApiResponse.Error.Network -> {
                // Retry later
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.debug("Network error syncing event ${event.sessionId}, will retry")
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                // Retry later (user might log back in)
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.debug("Unauthorized syncing event ${event.sessionId}, will retry")
            }
            RemoteApiResponse.Error.NotFound -> {
                // Keep the rejected record locally without repeatedly submitting it.
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.SyncError)
                logger.warn("Listening event ${event.sessionId} rejected permanently: $result")
            }
            else -> {
                val permanent = result is RemoteApiResponse.Error.Unknown && result.httpStatus == 400
                listeningEventStore.updateSyncStatus(event.id, if (permanent) SyncStatus.SyncError else SyncStatus.PendingSync)
                logger.error("Failed to sync listening event ${event.sessionId}: $result")
            }
        }
    }

    companion object {
        private val MIN_SLEEP_DURATION = 1.seconds
        private val MAX_SLEEP_DURATION = 30.seconds
        private const val CLEANUP_AGE_DAYS = 7L
    }
}
