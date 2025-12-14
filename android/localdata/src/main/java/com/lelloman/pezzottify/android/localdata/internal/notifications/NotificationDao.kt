package com.lelloman.pezzottify.android.localdata.internal.notifications

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import kotlinx.coroutines.flow.Flow

@Dao
internal interface NotificationDao {

    // ==================== Notifications ====================

    @Query("SELECT * FROM ${NotificationEntity.TABLE_NAME} ORDER BY ${NotificationEntity.COLUMN_CREATED_AT} DESC")
    fun getAll(): Flow<List<NotificationEntity>>

    @Query("SELECT * FROM ${NotificationEntity.TABLE_NAME} ORDER BY ${NotificationEntity.COLUMN_CREATED_AT} DESC")
    suspend fun getAllOnce(): List<NotificationEntity>

    @Query("SELECT COUNT(*) FROM ${NotificationEntity.TABLE_NAME} WHERE ${NotificationEntity.COLUMN_READ_AT} IS NULL")
    fun getUnreadCount(): Flow<Int>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(notification: NotificationEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(notifications: List<NotificationEntity>)

    @Query("UPDATE ${NotificationEntity.TABLE_NAME} SET ${NotificationEntity.COLUMN_READ_AT} = :readAt WHERE ${NotificationEntity.COLUMN_ID} = :notificationId")
    suspend fun markAsRead(notificationId: String, readAt: Long)

    @Query("DELETE FROM ${NotificationEntity.TABLE_NAME}")
    suspend fun deleteAll()

    @Query("DELETE FROM ${NotificationEntity.TABLE_NAME} WHERE ${NotificationEntity.COLUMN_ID} NOT IN (SELECT ${NotificationEntity.COLUMN_ID} FROM ${NotificationEntity.TABLE_NAME} ORDER BY ${NotificationEntity.COLUMN_CREATED_AT} DESC LIMIT :maxCount)")
    suspend fun deleteOldest(maxCount: Int)

    @Transaction
    suspend fun replaceAll(notifications: List<NotificationEntity>) {
        deleteAll()
        insertAll(notifications)
    }

    @Transaction
    suspend fun upsertAndTrim(notification: NotificationEntity, maxCount: Int) {
        upsert(notification)
        deleteOldest(maxCount)
    }

    // ==================== Pending Reads (Offline Queue) ====================

    @Query("SELECT * FROM ${PendingNotificationReadEntity.TABLE_NAME} ORDER BY ${PendingNotificationReadEntity.COLUMN_CREATED_AT} ASC")
    suspend fun getPendingReads(): List<PendingNotificationReadEntity>

    @Query("SELECT COUNT(*) FROM ${PendingNotificationReadEntity.TABLE_NAME}")
    suspend fun getPendingReadCount(): Int

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertPendingRead(pendingRead: PendingNotificationReadEntity)

    @Query("DELETE FROM ${PendingNotificationReadEntity.TABLE_NAME} WHERE ${PendingNotificationReadEntity.COLUMN_NOTIFICATION_ID} = :notificationId")
    suspend fun deletePendingRead(notificationId: String)

    @Query("UPDATE ${PendingNotificationReadEntity.TABLE_NAME} SET ${PendingNotificationReadEntity.COLUMN_RETRY_COUNT} = ${PendingNotificationReadEntity.COLUMN_RETRY_COUNT} + 1 WHERE ${PendingNotificationReadEntity.COLUMN_NOTIFICATION_ID} = :notificationId")
    suspend fun incrementRetryCount(notificationId: String)

    @Query("DELETE FROM ${PendingNotificationReadEntity.TABLE_NAME}")
    suspend fun deleteAllPendingReads()
}
