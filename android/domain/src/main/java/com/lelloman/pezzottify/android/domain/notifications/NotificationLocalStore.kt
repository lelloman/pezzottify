package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.coroutines.flow.Flow

/**
 * Local store for notifications with offline queue support.
 */
interface NotificationLocalStore {

    /**
     * Observe all notifications, ordered by creation time descending.
     */
    fun observeNotifications(): Flow<List<Notification>>

    /**
     * Observe the count of unread notifications.
     */
    fun observeUnreadCount(): Flow<Int>

    /**
     * Get all notifications once (suspend).
     */
    suspend fun getNotifications(): List<Notification>

    /**
     * Replace all notifications with the given list (used during full sync).
     */
    suspend fun replaceAll(notifications: List<Notification>)

    /**
     * Insert or update a single notification.
     */
    suspend fun upsert(notification: Notification)

    /**
     * Mark a notification as read locally.
     */
    suspend fun markAsReadLocally(notificationId: String, readAt: Long)

    /**
     * Mark all unread notifications as read locally.
     */
    suspend fun markAllAsReadLocally(readAt: Long)

    /**
     * Get IDs of all unread notifications.
     */
    suspend fun getUnreadIds(): List<String>

    /**
     * Clear all notifications.
     */
    suspend fun clear()

    // ==================== Pending Read Queue ====================

    /**
     * Data class representing a pending mark-as-read operation.
     */
    data class PendingRead(
        val notificationId: String,
        val readAt: Long,
        val createdAt: Long,
        val retryCount: Int,
    )

    /**
     * Add a mark-as-read operation to the pending queue.
     */
    suspend fun addPendingRead(notificationId: String, readAt: Long)

    /**
     * Get all pending mark-as-read operations.
     */
    suspend fun getPendingReads(): List<PendingRead>

    /**
     * Get the count of pending mark-as-read operations.
     */
    suspend fun getPendingReadCount(): Int

    /**
     * Remove a pending read from the queue (after successful sync).
     */
    suspend fun removePendingRead(notificationId: String)

    /**
     * Increment the retry count for a pending read (after failed sync attempt).
     */
    suspend fun incrementRetryCount(notificationId: String)

    /**
     * Clear all pending reads.
     */
    suspend fun clearPendingReads()
}
