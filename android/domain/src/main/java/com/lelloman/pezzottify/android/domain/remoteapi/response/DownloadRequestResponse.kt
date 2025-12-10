package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Status of a download queue item.
 */
@Serializable
enum class DownloadQueueStatus {
    @SerialName("PENDING")
    Pending,
    @SerialName("IN_PROGRESS")
    InProgress,
    @SerialName("RETRY_WAITING")
    RetryWaiting,
    @SerialName("COMPLETED")
    Completed,
    @SerialName("FAILED")
    Failed,
}

/**
 * Type of content being downloaded.
 */
@Serializable
enum class DownloadContentType {
    @SerialName("ALBUM")
    Album,
    @SerialName("TRACK_AUDIO")
    TrackAudio,
    @SerialName("ARTIST_IMAGE")
    ArtistImage,
    @SerialName("ALBUM_IMAGE")
    AlbumImage,
}

/**
 * Progress information for a download with children (e.g., album with tracks).
 */
@Serializable
data class DownloadProgress(
    /** Total number of child items */
    @SerialName("total_children")
    val totalChildren: Int,
    /** Number of completed children */
    val completed: Int,
    /** Number of failed children */
    val failed: Int,
    /** Number of pending children */
    val pending: Int,
    /** Number of in-progress children */
    @SerialName("in_progress")
    val inProgress: Int,
)

/**
 * A user's download request view (simplified queue item for user-facing API).
 */
@Serializable
data class DownloadRequestItem(
    /** Queue item ID */
    val id: String,
    /** Type of content being downloaded */
    @SerialName("content_type")
    val contentType: DownloadContentType,
    /** External content ID */
    @SerialName("content_id")
    val contentId: String,
    /** Display name (album/artist name) */
    @SerialName("content_name")
    val contentName: String,
    /** Artist name for display */
    @SerialName("artist_name")
    val artistName: String? = null,
    /** Current status */
    val status: DownloadQueueStatus,
    /** When the request was created (Unix timestamp) */
    @SerialName("created_at")
    val createdAt: Long,
    /** When the request completed (Unix timestamp) */
    @SerialName("completed_at")
    val completedAt: Long? = null,
    /** Error message if failed */
    @SerialName("error_message")
    val errorMessage: String? = null,
    /** Progress for album requests (shows child item status) */
    val progress: DownloadProgress? = null,
    /** Position in queue (for pending items) */
    @SerialName("queue_position")
    val queuePosition: Int? = null,
)

/**
 * Response for GET /v1/download/my-requests.
 */
@Serializable
data class MyDownloadRequestsResponse(
    /** List of user's download requests */
    val requests: List<DownloadRequestItem>,
    /** User's current rate limit status */
    val stats: DownloadLimitsResponse,
)
