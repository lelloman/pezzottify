package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/**
 * Type of notification.
 */
@Serializable
enum class NotificationType {
    @SerialName("download_completed")
    DownloadCompleted,
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}

/**
 * A user notification.
 */
@Serializable
data class Notification(
    val id: String,
    @SerialName("notification_type")
    val notificationType: NotificationType,
    val title: String,
    val body: String? = null,
    val data: JsonElement,
    @SerialName("read_at")
    val readAt: Long? = null,
    @SerialName("created_at")
    val createdAt: Long,
)

/**
 * Data payload for download_completed notifications.
 */
@Serializable
data class DownloadCompletedData(
    @SerialName("album_id")
    val albumId: String,
    @SerialName("album_name")
    val albumName: String,
    @SerialName("artist_name")
    val artistName: String,
    @SerialName("image_id")
    val imageId: String? = null,
    @SerialName("request_id")
    val requestId: String,
)

/**
 * Extension function to extract album ID from a notification's data payload.
 * Returns null if the notification type doesn't have an album ID or parsing fails.
 */
fun Notification.getAlbumId(): String? = when (notificationType) {
    NotificationType.DownloadCompleted -> {
        try {
            kotlinx.serialization.json.Json.decodeFromJsonElement(
                DownloadCompletedData.serializer(),
                data
            ).albumId
        } catch (_: Exception) {
            null
        }
    }
}
