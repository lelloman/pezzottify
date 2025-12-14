package com.lelloman.pezzottify.android.localdata.internal.notifications

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Entity for storing pending mark-as-read operations that need to be synced with the server.
 * Used for offline support - when user marks notification as read while offline,
 * the operation is queued here and synced when connection is restored.
 */
@Entity(tableName = PendingNotificationReadEntity.TABLE_NAME)
internal data class PendingNotificationReadEntity(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_NOTIFICATION_ID)
    val notificationId: String,

    @ColumnInfo(name = COLUMN_READ_AT)
    val readAt: Long,

    @ColumnInfo(name = COLUMN_CREATED_AT)
    val createdAt: Long,

    @ColumnInfo(name = COLUMN_RETRY_COUNT)
    val retryCount: Int = 0,
) {
    companion object {
        const val TABLE_NAME = "pending_notification_read"

        const val COLUMN_NOTIFICATION_ID = "notification_id"
        const val COLUMN_READ_AT = "read_at"
        const val COLUMN_CREATED_AT = "created_at"
        const val COLUMN_RETRY_COUNT = "retry_count"
    }
}
