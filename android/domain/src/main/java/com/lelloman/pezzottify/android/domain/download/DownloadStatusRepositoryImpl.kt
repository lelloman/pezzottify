package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadProgress
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus
import com.lelloman.pezzottify.android.domain.remoteapi.response.RequestStatusInfo
import com.lelloman.pezzottify.android.domain.sync.SyncDownloadProgress
import com.lelloman.pezzottify.android.domain.sync.SyncQueueStatus
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Implementation of DownloadStatusRepository that maintains in-memory status
 * and broadcasts updates via flows.
 */
@Singleton
class DownloadStatusRepositoryImpl @Inject constructor(
    loggerFactory: LoggerFactory,
) : DownloadStatusRepository {

    private val logger: Logger by loggerFactory

    // In-memory cache of request status by content ID
    private val statusCache = MutableStateFlow<Map<String, RequestStatusInfo>>(emptyMap())

    // Broadcast channel for all updates
    private val updatesFlow = MutableSharedFlow<DownloadStatusUpdate>(
        extraBufferCapacity = 64 // Buffer for observers that might be slow
    )

    override fun observeStatus(contentId: String): Flow<RequestStatusInfo?> {
        return statusCache.map { cache -> cache[contentId] }
    }

    override fun observeAllUpdates(): Flow<DownloadStatusUpdate> {
        return updatesFlow.asSharedFlow()
    }

    override suspend fun onRequestCreated(
        requestId: String,
        contentId: String,
        contentName: String,
        artistName: String?,
        queuePosition: Int,
    ) {
        logger.debug("onRequestCreated() $contentName by $artistName (content=$contentId, request=$requestId)")

        val status = RequestStatusInfo(
            requestId = requestId,
            status = DownloadQueueStatus.Pending,
            queuePosition = queuePosition,
            progress = null,
            errorMessage = null,
            createdAt = System.currentTimeMillis(),
        )

        updateCache(contentId, status)
        updatesFlow.emit(
            DownloadStatusUpdate.Created(
                requestId = requestId,
                contentId = contentId,
                contentName = contentName,
                artistName = artistName,
                queuePosition = queuePosition,
            )
        )
    }

    override suspend fun onStatusChanged(
        requestId: String,
        contentId: String,
        status: SyncQueueStatus,
        queuePosition: Int?,
        errorMessage: String?,
    ) {
        logger.debug("onStatusChanged() content=$contentId, status=$status, position=$queuePosition")

        val domainStatus = status.toDomainStatus()
        val existingStatus = statusCache.value[contentId]
        val updatedStatus = existingStatus?.copy(
            status = domainStatus,
            queuePosition = queuePosition,
            errorMessage = errorMessage,
        ) ?: RequestStatusInfo(
            requestId = requestId,
            status = domainStatus,
            queuePosition = queuePosition,
            progress = null,
            errorMessage = errorMessage,
            createdAt = System.currentTimeMillis(),
        )

        updateCache(contentId, updatedStatus)
        updatesFlow.emit(
            DownloadStatusUpdate.StatusChanged(
                requestId = requestId,
                contentId = contentId,
                status = domainStatus,
                queuePosition = queuePosition,
                errorMessage = errorMessage,
            )
        )
    }

    override suspend fun onProgressUpdated(
        requestId: String,
        contentId: String,
        progress: SyncDownloadProgress,
    ) {
        logger.debug("onProgressUpdated() content=$contentId, progress=${progress.completed}/${progress.totalChildren}")

        val domainProgress = progress.toDomainProgress()
        val existingStatus = statusCache.value[contentId]
        val updatedStatus = existingStatus?.copy(
            progress = domainProgress,
        ) ?: RequestStatusInfo(
            requestId = requestId,
            status = DownloadQueueStatus.InProgress,
            queuePosition = null,
            progress = domainProgress,
            errorMessage = null,
            createdAt = System.currentTimeMillis(),
        )

        updateCache(contentId, updatedStatus)
        updatesFlow.emit(
            DownloadStatusUpdate.ProgressUpdated(
                requestId = requestId,
                contentId = contentId,
                progress = domainProgress,
            )
        )
    }

    override suspend fun onCompleted(
        requestId: String,
        contentId: String,
    ) {
        logger.debug("onCompleted() content=$contentId")

        val existingStatus = statusCache.value[contentId]
        val updatedStatus = existingStatus?.copy(
            status = DownloadQueueStatus.Completed,
            queuePosition = null,
        ) ?: RequestStatusInfo(
            requestId = requestId,
            status = DownloadQueueStatus.Completed,
            queuePosition = null,
            progress = null,
            errorMessage = null,
            createdAt = System.currentTimeMillis(),
        )

        updateCache(contentId, updatedStatus)
        updatesFlow.emit(
            DownloadStatusUpdate.Completed(
                requestId = requestId,
                contentId = contentId,
            )
        )
    }

    override suspend fun clear() {
        logger.debug("clear()")
        statusCache.value = emptyMap()
    }

    private fun updateCache(contentId: String, status: RequestStatusInfo) {
        statusCache.value = statusCache.value + (contentId to status)
    }

    private fun SyncQueueStatus.toDomainStatus(): DownloadQueueStatus = when (this) {
        SyncQueueStatus.Pending -> DownloadQueueStatus.Pending
        SyncQueueStatus.InProgress -> DownloadQueueStatus.InProgress
        SyncQueueStatus.Completed -> DownloadQueueStatus.Completed
        SyncQueueStatus.Failed -> DownloadQueueStatus.Failed
        SyncQueueStatus.RetryWaiting -> DownloadQueueStatus.RetryWaiting
    }

    private fun SyncDownloadProgress.toDomainProgress(): DownloadProgress = DownloadProgress(
        totalChildren = totalChildren,
        completed = completed,
        failed = failed,
        pending = pending,
        inProgress = inProgress,
    )
}
