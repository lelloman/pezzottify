package com.lelloman.pezzottify.android.domain.catalogsync

import kotlinx.coroutines.flow.StateFlow

/**
 * Store for catalog sync cursor persistence.
 *
 * Tracks the last processed sequence number so clients can catch up
 * on missed events when reconnecting.
 */
interface CatalogSyncStore {
    /**
     * The current sequence number cursor.
     * Events with seq > currentSeq haven't been processed yet.
     */
    val currentSeq: StateFlow<Long>

    /**
     * Update the cursor to a new sequence number.
     */
    suspend fun setCurrentSeq(seq: Long)

    /**
     * Clear the stored cursor (e.g., on logout).
     */
    suspend fun clear()
}
