package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

/**
 * Repository interface for notification operations.
 */
interface NotificationRepository {
    /** All notifications, ordered by createdAt DESC */
    val notifications: StateFlow<List<Notification>>

    /** Count of unread notifications */
    val unreadCount: StateFlow<Int>

    /** Flow of real-time notification updates */
    fun observeUpdates(): Flow<NotificationUpdate>

    /** Called by SyncManager on full sync */
    suspend fun setNotifications(notifications: List<Notification>)

    /** Called by SyncManager when notification_created event received */
    suspend fun onNotificationCreated(notification: Notification)

    /** Called by SyncManager when notification_read event received */
    suspend fun onNotificationRead(notificationId: String, readAt: Long)

    /** Mark notification as read (triggers API call + local update) */
    suspend fun markAsRead(notificationId: String): Result<Unit>

    /** Clear all notifications (on logout) */
    suspend fun clear()

    /** Check if there are pending mark-as-read operations that need to be synced */
    suspend fun hasPendingReads(): Boolean

    /** Process pending mark-as-read operations. Returns number of successfully synced items. */
    suspend fun processPendingReads(): Int
}

/**
 * Real-time notification update event.
 */
sealed interface NotificationUpdate {
    data class Created(val notification: Notification) : NotificationUpdate
    data class Read(val notificationId: String, val readAt: Long) : NotificationUpdate
}
