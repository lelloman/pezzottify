package com.lelloman.pezzottify.android.domain.impression

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.seconds

/**
 * Background synchronizer that sends pending impressions to server.
 *
 * Extends BaseSynchronizer for consistent behavior with other synchronizers:
 * - Exponential backoff on failures
 * - Sleep/wake mechanism
 */
@Singleton
class ImpressionSynchronizer internal constructor(
    private val impressionStore: ImpressionStore,
    private val remoteApiClient: RemoteApiClient,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<Impression>(
    logger = loggerFactory.getLogger(ImpressionSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    @Inject
    constructor(
        impressionStore: ImpressionStore,
        remoteApiClient: RemoteApiClient,
        timeProvider: TimeProvider,
        loggerFactory: LoggerFactory,
    ) : this(
        impressionStore,
        remoteApiClient,
        timeProvider,
        loggerFactory,
        Dispatchers.IO,
        GlobalScope,
    )

    override suspend fun getItemsToProcess(): List<Impression> =
        impressionStore.getPendingSyncImpressions()

    override suspend fun processItem(item: Impression) {
        syncImpression(item)
    }

    override suspend fun onBeforeMainLoop() {
        // Cleanup old non-synced impressions on app startup
        val cutoff = timeProvider.nowUtcMs() - TimeUnit.DAYS.toMillis(CLEANUP_AGE_DAYS)
        val deleted = impressionStore.deleteOldNonSyncedImpressions(cutoff)
        if (deleted > 0) {
            logger.info("Cleaned up $deleted old non-synced impressions")
        }
        // Also delete already synced impressions - we don't need to keep them locally
        val deletedSynced = impressionStore.deleteSyncedImpressions()
        if (deletedSynced > 0) {
            logger.info("Cleaned up $deletedSynced synced impressions")
        }
    }

    private suspend fun syncImpression(impression: Impression) {
        impressionStore.updateSyncStatus(impression.id, SyncStatus.Syncing)

        val result = remoteApiClient.recordImpression(
            itemType = impression.itemType.name.lowercase(),
            itemId = impression.itemId,
        )

        when (result) {
            is RemoteApiResponse.Success -> {
                // Delete after successful sync - impressions don't need to be kept locally
                impressionStore.deleteImpression(impression.id)
                logger.debug("Successfully synced impression ${impression.itemType}:${impression.itemId}")
            }
            is RemoteApiResponse.Error.Network -> {
                // Retry later
                impressionStore.updateSyncStatus(impression.id, SyncStatus.PendingSync)
                logger.debug("Network error syncing impression, will retry")
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                // Retry later (user might log back in)
                impressionStore.updateSyncStatus(impression.id, SyncStatus.PendingSync)
                logger.debug("Unauthorized syncing impression, will retry")
            }
            else -> {
                // Retry later (conform to existing pattern)
                impressionStore.updateSyncStatus(impression.id, SyncStatus.PendingSync)
                logger.error("Failed to sync impression: $result")
            }
        }
    }

    companion object {
        private val MIN_SLEEP_DURATION = 1.seconds
        private val MAX_SLEEP_DURATION = 30.seconds
        private const val CLEANUP_AGE_DAYS = 7L
    }
}
