package com.lelloman.pezzottify.android.localdata.internal.notifications

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.notifications.Notification
import com.lelloman.pezzottify.android.domain.notifications.NotificationType
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement

@Entity(
    tableName = NotificationEntity.TABLE_NAME,
    indices = [Index(value = ["created_at"], name = "index_notification_created_at")]
)
internal data class NotificationEntity(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_ID)
    val id: String,

    @ColumnInfo(name = COLUMN_NOTIFICATION_TYPE)
    val notificationType: NotificationType,

    @ColumnInfo(name = COLUMN_TITLE)
    val title: String,

    @ColumnInfo(name = COLUMN_BODY)
    val body: String?,

    @ColumnInfo(name = COLUMN_DATA)
    val data: String, // JSON string

    @ColumnInfo(name = COLUMN_READ_AT)
    val readAt: Long?,

    @ColumnInfo(name = COLUMN_CREATED_AT)
    val createdAt: Long,
) {
    companion object {
        const val TABLE_NAME = "notification"

        const val COLUMN_ID = "id"
        const val COLUMN_NOTIFICATION_TYPE = "notification_type"
        const val COLUMN_TITLE = "title"
        const val COLUMN_BODY = "body"
        const val COLUMN_DATA = "data"
        const val COLUMN_READ_AT = "read_at"
        const val COLUMN_CREATED_AT = "created_at"
    }

    fun toDomain(): Notification = Notification(
        id = id,
        notificationType = notificationType,
        title = title,
        body = body,
        data = Json.parseToJsonElement(data),
        readAt = readAt,
        createdAt = createdAt,
    )
}

internal fun Notification.toEntity() = NotificationEntity(
    id = id,
    notificationType = notificationType,
    title = title,
    body = body,
    data = Json.encodeToString(JsonElement.serializer(), data),
    readAt = readAt,
    createdAt = createdAt,
)
