package com.lelloman.pezzottify.android.domain.sync

import kotlinx.coroutines.flow.StateFlow

/**
 * Store for sync state persistence.
 *
 * Manages the sync cursor (sequence number) to track which events
 * have been processed. This enables efficient catch-up sync when
 * the app restarts or reconnects.
 */
interface SyncStateStore {

    /**
     * Observable sync cursor value.
     * The cursor represents the last successfully processed event sequence number.
     * A value of 0 means no events have been processed (fresh sync needed).
     */
    val cursor: StateFlow<Long>

    /**
     * Get the current sync cursor value.
     * Returns 0 if no cursor has been saved.
     */
    fun getCursor(): Long

    /**
     * Save a new sync cursor value.
     * This should be called after successfully processing a sync event.
     */
    suspend fun saveCursor(cursor: Long)

    /**
     * Clear the sync cursor.
     * This should be called on logout or when a full sync is required.
     */
    suspend fun clearCursor()
}
