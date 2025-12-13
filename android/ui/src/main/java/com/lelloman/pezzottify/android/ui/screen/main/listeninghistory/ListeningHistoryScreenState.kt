package com.lelloman.pezzottify.android.ui.screen.main.listeninghistory

data class ListeningHistoryScreenState(
    val isLoading: Boolean = false,
    val events: List<UiListeningEvent> = emptyList(),
    val errorRes: Int? = null,
    val hasMorePages: Boolean = false,
)

data class UiListeningEvent(
    val id: Long,
    val trackId: String,
    val startedAt: Long,
    val durationSeconds: Int,
    val trackDurationSeconds: Int,
    val completed: Boolean,
    val playbackContext: String?,
    val clientType: String?,
)

enum class ListeningHistoryErrorType {
    Network,
    Unauthorized,
    Unknown,
}

class ListeningHistoryException(val errorType: ListeningHistoryErrorType) : Exception()
