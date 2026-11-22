package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonElement

/**
 * Type of notification.
 *
 * Unrecognised values decode to [Unknown] so that a server adding a new type
 * never breaks sync for older app versions.
 */
@Serializable(with = NotificationTypeSerializer::class)
enum class NotificationType(val wireName: String) {
    DownloadCompleted("download_completed"),
    BugReportSubmitted("bug_report_submitted"),
    Unknown("unknown"),
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}

object NotificationTypeSerializer : KSerializer<NotificationType> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("NotificationType", PrimitiveKind.STRING)

    override fun serialize(encoder: Encoder, value: NotificationType) =
        encoder.encodeString(value.wireName)

    override fun deserialize(decoder: Decoder): NotificationType {
        val name = decoder.decodeString()
        return NotificationType.entries.firstOrNull { it.wireName == name }
            ?: NotificationType.Unknown
    }
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
 * Data payload for bug_report_submitted notifications, sent to report triagers.
 */
@Serializable
data class BugReportSubmittedData(
    @SerialName("report_id")
    val reportId: String,
    val kind: String,
    val category: String,
    val title: String? = null,
    @SerialName("reporter_handle")
    val reporterHandle: String,
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
    NotificationType.BugReportSubmitted, NotificationType.Unknown -> null
}
