package com.lelloman.pezzottify.android.localdata.internal.notifications

import com.lelloman.pezzottify.android.domain.notifications.Notification
import com.lelloman.pezzottify.android.domain.notifications.NotificationLocalStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class NotificationLocalStoreImpl @Inject constructor(
    private val notificationDao: NotificationDao,
) : NotificationLocalStore {

    companion object {
        private const val MAX_NOTIFICATIONS = 100
    }

    override fun observeNotifications(): Flow<List<Notification>> =
        notificationDao.getAll().map { entities ->
            entities.map { it.toDomain() }
        }

    override fun observeUnreadCount(): Flow<Int> =
        notificationDao.getUnreadCount()

    override suspend fun getNotifications(): List<Notification> =
        notificationDao.getAllOnce().map { it.toDomain() }

    override suspend fun replaceAll(notifications: List<Notification>) {
        notificationDao.replaceAll(notifications.map { it.toEntity() })
    }

    override suspend fun upsert(notification: Notification) {
        notificationDao.upsertAndTrim(notification.toEntity(), MAX_NOTIFICATIONS)
    }

    override suspend fun markAsReadLocally(notificationId: String, readAt: Long) {
        notificationDao.markAsRead(notificationId, readAt)
    }

    override suspend fun markAllAsReadLocally(readAt: Long) {
        notificationDao.markAllAsRead(readAt)
    }

    override suspend fun getUnreadIds(): List<String> =
        notificationDao.getUnreadIds()

    override suspend fun clear() {
        notificationDao.deleteAll()
        notificationDao.deleteAllPendingReads()
    }

    // ==================== Pending Read Queue ====================

    override suspend fun addPendingRead(notificationId: String, readAt: Long) {
        notificationDao.insertPendingRead(
            PendingNotificationReadEntity(
                notificationId = notificationId,
                readAt = readAt,
                createdAt = System.currentTimeMillis(),
                retryCount = 0,
            )
        )
    }

    override suspend fun getPendingReads(): List<NotificationLocalStore.PendingRead> =
        notificationDao.getPendingReads().map { entity ->
            NotificationLocalStore.PendingRead(
                notificationId = entity.notificationId,
                readAt = entity.readAt,
                createdAt = entity.createdAt,
                retryCount = entity.retryCount,
            )
        }

    override suspend fun getPendingReadCount(): Int =
        notificationDao.getPendingReadCount()

    override suspend fun removePendingRead(notificationId: String) {
        notificationDao.deletePendingRead(notificationId)
    }

    override suspend fun incrementRetryCount(notificationId: String) {
        notificationDao.incrementRetryCount(notificationId)
    }

    override suspend fun clearPendingReads() {
        notificationDao.deleteAllPendingReads()
    }
}
