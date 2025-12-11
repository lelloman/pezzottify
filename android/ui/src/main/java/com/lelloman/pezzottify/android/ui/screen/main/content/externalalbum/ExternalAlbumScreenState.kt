package com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum

import androidx.annotation.StringRes

/**
 * UI state for the external album screen.
 */
data class ExternalAlbumScreenState(
    val isLoading: Boolean = true,
    val album: UiExternalAlbumWithStatus? = null,
    val requestStatus: UiRequestStatus? = null,
    val isRequesting: Boolean = false,
    @StringRes val errorRes: Int? = null,
    val errorMessage: String? = null,
)

/**
 * UI model for external album details with request status.
 */
data class UiExternalAlbumWithStatus(
    val id: String,
    val name: String,
    val artistId: String,
    val artistName: String,
    val imageUrl: String?,
    val year: Int?,
    val albumType: String?,
    val totalTracks: Int,
    val tracks: List<UiExternalTrack>,
    val inCatalog: Boolean,
    val requestStatus: UiRequestStatus?,
)

/**
 * UI model for external track info.
 */
data class UiExternalTrack(
    val id: String,
    val name: String,
    val trackNumber: Int,
    val durationMs: Long?,
)

/**
 * UI model for request status.
 */
data class UiRequestStatus(
    val requestId: String,
    val status: UiDownloadStatus,
    val queuePosition: Int?,
    val progress: UiDownloadProgress?,
    val errorMessage: String?,
)

/**
 * Download progress for UI.
 */
data class UiDownloadProgress(
    val completed: Int,
    val total: Int,
) {
    val percent: Float get() = if (total > 0) completed.toFloat() / total else 0f
}

/**
 * Download status enum for UI.
 */
enum class UiDownloadStatus {
    Pending,
    InProgress,
    RetryWaiting,
    Completed,
    Failed,
}

/**
 * Actions available on the external album screen.
 */
interface ExternalAlbumScreenActions {
    fun requestDownload()
    fun navigateToArtist()
    fun navigateToCatalogAlbum()
    fun retry()
}
