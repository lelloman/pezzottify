package com.lelloman.pezzottify.android.ui.screen.main.myrequests

import androidx.annotation.StringRes

data class MyRequestsScreenState(
    val requests: List<UiDownloadRequest>? = null,
    val isLoading: Boolean = true,
    @StringRes val errorRes: Int? = null,
    val selectedTab: MyRequestsTab = MyRequestsTab.Queue,
    val hasMoreCompleted: Boolean = true,
    val isLoadingMore: Boolean = false,
)

enum class MyRequestsTab {
    Queue,
    Completed,
}

data class UiDownloadRequest(
    val id: String,
    val albumName: String,
    val artistName: String,
    val status: RequestStatus,
    val progress: RequestProgress? = null,
    val errorMessage: String? = null,
    val catalogId: String? = null,
    val createdAt: Long = 0L,
    val completedAt: Long? = null,
    val queuePosition: Int? = null,
)

enum class RequestStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

data class RequestProgress(
    val current: Int,
    val total: Int,
) {
    val percent: Float get() = if (total > 0) current.toFloat() / total else 0f
}

/**
 * UI-specific sealed class representing download status updates.
 * Mirrors the domain DownloadStatusUpdate but keeps UI module independent of domain.
 */
sealed class UiDownloadStatusUpdate {
    data class Created(
        val requestId: String,
        val contentId: String,
        val contentName: String,
        val artistName: String?,
        val queuePosition: Int,
    ) : UiDownloadStatusUpdate()

    data class StatusChanged(
        val requestId: String,
        val status: RequestStatus,
        val queuePosition: Int?,
        val errorMessage: String?,
    ) : UiDownloadStatusUpdate()

    data class ProgressUpdated(
        val requestId: String,
        val completed: Int,
        val total: Int,
    ) : UiDownloadStatusUpdate()

    data class Completed(
        val requestId: String,
    ) : UiDownloadStatusUpdate()
}
