package com.lelloman.pezzottify.android.domain.notifications

/**
 * Interface for showing Android system notifications.
 * This interface is defined in the domain layer but implemented in the app layer
 * where we have access to Android Context.
 */
interface SystemNotificationHelper {
    /**
     * Show a notification when a new catalog batch is closed.
     *
     * @param batchId The ID of the closed batch
     * @param batchName The name of the batch
     * @param description Optional description of the batch
     * @param albumsAdded Number of albums added
     * @param artistsAdded Number of artists added
     * @param tracksAdded Number of tracks added
     */
    fun showWhatsNewNotification(
        batchId: String,
        batchName: String,
        description: String?,
        albumsAdded: Int,
        artistsAdded: Int,
        tracksAdded: Int,
    )

    /**
     * Show a notification when a requested album download has completed.
     *
     * @param albumId The ID of the downloaded album (for navigation)
     * @param albumName The name of the album
     * @param artistName The name of the artist
     */
    fun showDownloadCompletedNotification(
        albumId: String,
        albumName: String,
        artistName: String,
    )
}
