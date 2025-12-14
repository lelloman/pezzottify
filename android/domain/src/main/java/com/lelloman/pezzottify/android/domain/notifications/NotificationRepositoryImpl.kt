package com.lelloman.pezzottify.android.domain.notifications

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Implementation of NotificationRepository that uses Room for persistence
 * and maintains an offline queue for mark-as-read operations.
 */
@Singleton
class NotificationRepositoryImpl @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val localStore: NotificationLocalStore,
    loggerFactory: LoggerFactory,
) : NotificationRepository {

    private val logger: Logger by loggerFactory

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override val notifications: StateFlow<List<Notification>> = localStore
        .observeNotifications()
        .stateIn(scope, SharingStarted.Eagerly, emptyList())

    override val unreadCount: StateFlow<Int> = localStore
        .observeUnreadCount()
        .stateIn(scope, SharingStarted.Eagerly, 0)

    private val _updates = MutableSharedFlow<NotificationUpdate>(extraBufferCapacity = 64)
    override fun observeUpdates(): Flow<NotificationUpdate> = _updates.asSharedFlow()

    override suspend fun setNotifications(notifications: List<Notification>) {
        logger.debug("setNotifications() count=${notifications.size}")
        localStore.replaceAll(notifications)
    }

    override suspend fun onNotificationCreated(notification: Notification) {
        logger.debug("onNotificationCreated() id=${notification.id} title=${notification.title}")
        localStore.upsert(notification)
        _updates.emit(NotificationUpdate.Created(notification))
    }

    override suspend fun onNotificationRead(notificationId: String, readAt: Long) {
        logger.debug("onNotificationRead() id=$notificationId readAt=$readAt")
        localStore.markAsReadLocally(notificationId, readAt)
        // Also remove from pending queue if it was there (server confirmed the read)
        localStore.removePendingRead(notificationId)
        _updates.emit(NotificationUpdate.Read(notificationId, readAt))
    }

    override suspend fun markAsRead(notificationId: String): Result<Unit> {
        logger.debug("markAsRead() id=$notificationId")

        val readAt = System.currentTimeMillis() / 1000

        // Optimistic update - mark as read locally immediately
        localStore.markAsReadLocally(notificationId, readAt)
        _updates.emit(NotificationUpdate.Read(notificationId, readAt))

        // Try to sync with server
        return when (val response = remoteApiClient.markNotificationRead(notificationId)) {
            is RemoteApiResponse.Success -> {
                logger.debug("markAsRead() synced successfully")
                Result.success(Unit)
            }
            is RemoteApiResponse.Error -> {
                logger.warn("markAsRead() failed, queueing for later sync: $response")
                // Add to pending queue for later sync
                localStore.addPendingRead(notificationId, readAt)
                // Return success since local update succeeded - will sync later
                Result.success(Unit)
            }
        }
    }

    override suspend fun clear() {
        logger.debug("clear()")
        localStore.clear()
    }

    /**
     * Process pending mark-as-read operations. Called by SyncManager when connection is restored.
     * @return Number of successfully synced items
     */
    override suspend fun processPendingReads(): Int {
        val pendingReads = localStore.getPendingReads()
        if (pendingReads.isEmpty()) {
            return 0
        }

        logger.debug("processPendingReads() processing ${pendingReads.size} pending items")

        var successCount = 0
        for (pending in pendingReads) {
            when (val response = remoteApiClient.markNotificationRead(pending.notificationId)) {
                is RemoteApiResponse.Success -> {
                    logger.debug("processPendingReads() synced ${pending.notificationId}")
                    localStore.removePendingRead(pending.notificationId)
                    successCount++
                }
                is RemoteApiResponse.Error -> {
                    logger.warn("processPendingReads() failed for ${pending.notificationId}: $response")
                    if (pending.retryCount >= MAX_RETRY_COUNT) {
                        logger.warn("processPendingReads() max retries reached, removing ${pending.notificationId}")
                        localStore.removePendingRead(pending.notificationId)
                    } else {
                        localStore.incrementRetryCount(pending.notificationId)
                    }
                }
            }
        }

        logger.debug("processPendingReads() completed: $successCount/${pendingReads.size} synced")
        return successCount
    }

    /**
     * Check if there are pending mark-as-read operations that need to be synced.
     */
    override suspend fun hasPendingReads(): Boolean = localStore.getPendingReadCount() > 0

    companion object {
        private const val MAX_RETRY_COUNT = 5
    }
}
