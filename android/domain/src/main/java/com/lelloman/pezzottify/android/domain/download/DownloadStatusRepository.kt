package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadProgress
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus
import com.lelloman.pezzottify.android.domain.sync.SyncDownloadProgress
import com.lelloman.pezzottify.android.domain.sync.SyncQueueStatus
import kotlinx.coroutines.flow.Flow

/**
 * Download request status information tracked via sync events.
 * Used for updating UI with real-time download status.
 */
data class RequestStatusInfo(
    val requestId: String,
    val status: DownloadQueueStatus,
    val queuePosition: Int?,
    val progress: DownloadProgress?,
    val errorMessage: String?,
    val createdAt: Long,
)

/**
 * Repository that exposes download status updates from sync events.
 * Fed by SyncManager when download-related events arrive via WebSocket.
 */
interface DownloadStatusRepository {

    /**
     * Observe status updates for a specific content ID.
     * Emits whenever a sync event updates this content's status.
     * Returns null if there's no known status for this content.
     */
    fun observeStatus(contentId: String): Flow<RequestStatusInfo?>

    /**
     * Observe all download status updates (for My Requests screen).
     * Emits each update as it arrives via sync events.
     */
    fun observeAllUpdates(): Flow<DownloadStatusUpdate>

    /**
     * Called by SyncManager when a download_request_created event is received.
     */
    suspend fun onRequestCreated(
        requestId: String,
        contentId: String,
        contentName: String,
        artistName: String?,
        queuePosition: Int,
    )

    /**
     * Called by SyncManager when a download_status_changed event is received.
     */
    suspend fun onStatusChanged(
        requestId: String,
        contentId: String,
        status: SyncQueueStatus,
        queuePosition: Int?,
        errorMessage: String?,
    )

    /**
     * Called by SyncManager when a download_progress_updated event is received.
     */
    suspend fun onProgressUpdated(
        requestId: String,
        contentId: String,
        progress: SyncDownloadProgress,
    )

    /**
     * Called by SyncManager when a download_completed event is received.
     */
    suspend fun onCompleted(
        requestId: String,
        contentId: String,
    )

    /**
     * Clear all cached status data.
     * Called on logout.
     */
    suspend fun clear()
}

/**
 * Sealed class representing download status updates from sync events.
 */
sealed class DownloadStatusUpdate {
    /**
     * A new download request was created.
     */
    data class Created(
        val requestId: String,
        val contentId: String,
        val contentName: String,
        val artistName: String?,
        val queuePosition: Int,
    ) : DownloadStatusUpdate()

    /**
     * Download status changed (pending -> in_progress -> completed/failed).
     */
    data class StatusChanged(
        val requestId: String,
        val contentId: String,
        val status: DownloadQueueStatus,
        val queuePosition: Int?,
        val errorMessage: String?,
    ) : DownloadStatusUpdate()

    /**
     * Download progress updated (for album downloads with multiple tracks).
     */
    data class ProgressUpdated(
        val requestId: String,
        val contentId: String,
        val progress: DownloadProgress,
    ) : DownloadStatusUpdate()

    /**
     * Download completed successfully.
     */
    data class Completed(
        val requestId: String,
        val contentId: String,
    ) : DownloadStatusUpdate()
}
