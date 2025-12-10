package com.lelloman.pezzottify.android.ui.screen.main.myrequests

data class MyRequestsScreenState(
    val requests: List<UiDownloadRequest>? = null,
    val isLoading: Boolean = true,
    val error: String? = null,
    val limits: UiRequestLimits? = null,
)

data class UiDownloadRequest(
    val id: String,
    val albumName: String,
    val artistName: String,
    val status: RequestStatus,
    val progress: RequestProgress? = null,
    val errorMessage: String? = null,
    val catalogId: String? = null,
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

data class UiRequestLimits(
    val requestsToday: Int,
    val maxPerDay: Int,
    val inQueue: Int,
    val maxQueue: Int,
) {
    val isAtDailyLimit: Boolean get() = requestsToday >= maxPerDay
    val isAtQueueLimit: Boolean get() = inQueue >= maxQueue
}
