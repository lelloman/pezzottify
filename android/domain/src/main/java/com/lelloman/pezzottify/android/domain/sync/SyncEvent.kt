package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.notifications.Notification
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * Content type for liked/unliked events.
 */
@Serializable
enum class LikedContentType {
    @SerialName("album")
    Album,
    @SerialName("artist")
    Artist,
    @SerialName("track")
    Track,
}

/**
 * User setting types for setting_changed events.
 * Uses tagged serialization: {"key": "setting_key", "value": value}
 */
@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("key")
@Serializable
sealed interface UserSetting {
    @Serializable
    @SerialName("enable_external_search")
    data class ExternalSearchEnabled(val value: Boolean) : UserSetting
}

/**
 * User permissions that can be granted/revoked.
 */
@Serializable
enum class Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    ServerAdmin,
    ViewAnalytics,
    RequestContent,
    DownloadManagerAdmin,
}

// =============================================================================
// Download Status Types (for sync events)
// =============================================================================

/**
 * Download content type for sync events.
 */
@Serializable
enum class SyncDownloadContentType {
    @SerialName("album")
    Album,
}

/**
 * Download queue status for sync events.
 */
@Serializable
enum class SyncQueueStatus {
    @SerialName("PENDING")
    Pending,
    @SerialName("IN_PROGRESS")
    InProgress,
    @SerialName("COMPLETED")
    Completed,
    @SerialName("FAILED")
    Failed,
    @SerialName("RETRY_WAITING")
    RetryWaiting,
}

/**
 * Download progress for sync events.
 */
@Serializable
data class SyncDownloadProgress(
    @SerialName("total_children")
    val totalChildren: Int,
    val completed: Int,
    val failed: Int,
    val pending: Int,
    @SerialName("in_progress")
    val inProgress: Int,
)

/**
 * Sync event types for multi-device synchronization.
 *
 * Events are serialized using adjacently tagged representation:
 * `{"type": "event_name", "payload": {...}}`
 */
@Serializable
sealed interface SyncEvent {

    /**
     * Content (album, artist, track) was liked.
     */
    @Serializable
    @SerialName("content_liked")
    data class ContentLiked(
        @SerialName("content_type")
        val contentType: LikedContentType,
        @SerialName("content_id")
        val contentId: String,
    ) : SyncEvent

    /**
     * Content (album, artist, track) was unliked.
     */
    @Serializable
    @SerialName("content_unliked")
    data class ContentUnliked(
        @SerialName("content_type")
        val contentType: LikedContentType,
        @SerialName("content_id")
        val contentId: String,
    ) : SyncEvent

    /**
     * A user setting was changed.
     */
    @Serializable
    @SerialName("setting_changed")
    data class SettingChanged(
        val setting: UserSetting,
    ) : SyncEvent

    /**
     * A playlist was created.
     */
    @Serializable
    @SerialName("playlist_created")
    data class PlaylistCreated(
        @SerialName("playlist_id")
        val playlistId: String,
        val name: String,
    ) : SyncEvent

    /**
     * A playlist was renamed.
     */
    @Serializable
    @SerialName("playlist_renamed")
    data class PlaylistRenamed(
        @SerialName("playlist_id")
        val playlistId: String,
        val name: String,
    ) : SyncEvent

    /**
     * A playlist was deleted.
     */
    @Serializable
    @SerialName("playlist_deleted")
    data class PlaylistDeleted(
        @SerialName("playlist_id")
        val playlistId: String,
    ) : SyncEvent

    /**
     * Playlist tracks were updated (added or removed).
     */
    @Serializable
    @SerialName("playlist_tracks_updated")
    data class PlaylistTracksUpdated(
        @SerialName("playlist_id")
        val playlistId: String,
        @SerialName("track_ids")
        val trackIds: List<String>,
    ) : SyncEvent

    /**
     * A permission was granted to the user.
     */
    @Serializable
    @SerialName("permission_granted")
    data class PermissionGranted(
        val permission: Permission,
    ) : SyncEvent

    /**
     * A permission was revoked from the user.
     */
    @Serializable
    @SerialName("permission_revoked")
    data class PermissionRevoked(
        val permission: Permission,
    ) : SyncEvent

    /**
     * All permissions were reset (typically via admin action).
     */
    @Serializable
    @SerialName("permissions_reset")
    data class PermissionsReset(
        val permissions: List<Permission>,
    ) : SyncEvent

    // Download status events

    /**
     * A download request was created (album added to queue).
     */
    @Serializable
    @SerialName("download_request_created")
    data class DownloadRequestCreated(
        @SerialName("request_id")
        val requestId: String,
        @SerialName("content_id")
        val contentId: String,
        @SerialName("content_type")
        val contentType: SyncDownloadContentType,
        @SerialName("content_name")
        val contentName: String,
        @SerialName("artist_name")
        val artistName: String?,
        @SerialName("queue_position")
        val queuePosition: Int,
    ) : SyncEvent

    /**
     * Download status changed (e.g., pending -> in_progress -> completed/failed).
     */
    @Serializable
    @SerialName("download_status_changed")
    data class DownloadStatusChanged(
        @SerialName("request_id")
        val requestId: String,
        @SerialName("content_id")
        val contentId: String,
        val status: SyncQueueStatus,
        @SerialName("queue_position")
        val queuePosition: Int?,
        @SerialName("error_message")
        val errorMessage: String?,
    ) : SyncEvent

    /**
     * Download progress updated (for album downloads with multiple tracks).
     */
    @Serializable
    @SerialName("download_progress_updated")
    data class DownloadProgressUpdated(
        @SerialName("request_id")
        val requestId: String,
        @SerialName("content_id")
        val contentId: String,
        val progress: SyncDownloadProgress,
    ) : SyncEvent

    /**
     * Download completed successfully.
     */
    @Serializable
    @SerialName("download_completed")
    data class DownloadCompleted(
        @SerialName("request_id")
        val requestId: String,
        @SerialName("content_id")
        val contentId: String,
    ) : SyncEvent

    // Notification events

    /**
     * A notification was created.
     */
    @Serializable
    @SerialName("notification_created")
    data class NotificationCreated(
        val notification: Notification,
    ) : SyncEvent

    /**
     * A notification was marked as read.
     */
    @Serializable
    @SerialName("notification_read")
    data class NotificationRead(
        @SerialName("notification_id")
        val notificationId: String,
        @SerialName("read_at")
        val readAt: Long,
    ) : SyncEvent
}

/**
 * A sync event stored in the database with its sequence number and timestamp.
 *
 * The event field is flattened into this object, so the JSON structure is:
 * `{"seq": 42, "type": "content_liked", "payload": {...}, "server_timestamp": 1701700000}`
 */
@Serializable
data class StoredEvent(
    val seq: Long,
    val type: String,
    val payload: SyncEventPayload,
    @SerialName("server_timestamp")
    val serverTimestamp: Long,
)

/**
 * Payload union for all sync event types.
 * This is needed because kotlinx.serialization doesn't support @JsonClassDiscriminator
 * on flattened sealed classes in the same way as serde's flatten.
 */
@Serializable
data class SyncEventPayload(
    // ContentLiked / ContentUnliked
    @SerialName("content_type")
    val contentType: LikedContentType? = null,
    @SerialName("content_id")
    val contentId: String? = null,

    // SettingChanged
    val setting: UserSetting? = null,

    // Playlist events
    @SerialName("playlist_id")
    val playlistId: String? = null,
    val name: String? = null,
    @SerialName("track_ids")
    val trackIds: List<String>? = null,

    // Permission events
    val permission: Permission? = null,
    val permissions: List<Permission>? = null,

    // Download events
    // Note: Download events also use content_type and content_id fields above
    // Since SyncDownloadContentType.Album maps to "album" same as LikedContentType.Album,
    // we reuse contentType and convert in toSyncEvent
    @SerialName("request_id")
    val requestId: String? = null,
    @SerialName("content_name")
    val contentName: String? = null,
    @SerialName("artist_name")
    val artistName: String? = null,
    @SerialName("queue_position")
    val queuePosition: Int? = null,
    val status: SyncQueueStatus? = null,
    @SerialName("error_message")
    val errorMessage: String? = null,
    val progress: SyncDownloadProgress? = null,

    // Notification events
    val notification: Notification? = null,
    @SerialName("notification_id")
    val notificationId: String? = null,
    @SerialName("read_at")
    val readAt: Long? = null,
) {
    /**
     * Convert payload to a typed SyncEvent based on the event type.
     */
    fun toSyncEvent(type: String): SyncEvent? = when (type) {
        "content_liked" -> {
            if (contentType != null && contentId != null) {
                SyncEvent.ContentLiked(contentType, contentId)
            } else null
        }
        "content_unliked" -> {
            if (contentType != null && contentId != null) {
                SyncEvent.ContentUnliked(contentType, contentId)
            } else null
        }
        "setting_changed" -> {
            if (setting != null) {
                SyncEvent.SettingChanged(setting)
            } else null
        }
        "playlist_created" -> {
            if (playlistId != null && name != null) {
                SyncEvent.PlaylistCreated(playlistId, name)
            } else null
        }
        "playlist_renamed" -> {
            if (playlistId != null && name != null) {
                SyncEvent.PlaylistRenamed(playlistId, name)
            } else null
        }
        "playlist_deleted" -> {
            if (playlistId != null) {
                SyncEvent.PlaylistDeleted(playlistId)
            } else null
        }
        "playlist_tracks_updated" -> {
            if (playlistId != null && trackIds != null) {
                SyncEvent.PlaylistTracksUpdated(playlistId, trackIds)
            } else null
        }
        "permission_granted" -> {
            if (permission != null) {
                SyncEvent.PermissionGranted(permission)
            } else null
        }
        "permission_revoked" -> {
            if (permission != null) {
                SyncEvent.PermissionRevoked(permission)
            } else null
        }
        "permissions_reset" -> {
            if (permissions != null) {
                SyncEvent.PermissionsReset(permissions)
            } else null
        }
        "download_request_created" -> {
            // contentType is reused from liked events - Album maps to SyncDownloadContentType.Album
            val downloadType = when (contentType) {
                LikedContentType.Album -> SyncDownloadContentType.Album
                else -> null
            }
            if (requestId != null && contentId != null && downloadType != null &&
                contentName != null && queuePosition != null) {
                SyncEvent.DownloadRequestCreated(
                    requestId, contentId, downloadType, contentName, artistName, queuePosition
                )
            } else null
        }
        "download_status_changed" -> {
            if (requestId != null && contentId != null && status != null) {
                SyncEvent.DownloadStatusChanged(
                    requestId, contentId, status, queuePosition, errorMessage
                )
            } else null
        }
        "download_progress_updated" -> {
            if (requestId != null && contentId != null && progress != null) {
                SyncEvent.DownloadProgressUpdated(requestId, contentId, progress)
            } else null
        }
        "download_completed" -> {
            if (requestId != null && contentId != null) {
                SyncEvent.DownloadCompleted(requestId, contentId)
            } else null
        }
        "notification_created" -> {
            if (notification != null) {
                SyncEvent.NotificationCreated(notification)
            } else null
        }
        "notification_read" -> {
            if (notificationId != null && readAt != null) {
                SyncEvent.NotificationRead(notificationId, readAt)
            } else null
        }
        else -> null
    }
}

/**
 * Extension function to get the typed SyncEvent from a StoredEvent.
 */
fun StoredEvent.toSyncEvent(): SyncEvent? = payload.toSyncEvent(type)
