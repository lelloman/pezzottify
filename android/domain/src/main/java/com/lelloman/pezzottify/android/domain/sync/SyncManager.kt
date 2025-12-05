package com.lelloman.pezzottify.android.domain.sync

import kotlinx.coroutines.flow.StateFlow

/**
 * Sync state variants.
 */
sealed interface SyncState {
    /**
     * Sync is idle, waiting for triggers.
     */
    data object Idle : SyncState

    /**
     * Sync is in progress.
     */
    data object Syncing : SyncState

    /**
     * Sync completed successfully.
     */
    data class Synced(val cursor: Long) : SyncState

    /**
     * Sync encountered an error.
     */
    data class Error(val message: String) : SyncState
}

/**
 * Manager for multi-device sync operations.
 *
 * Handles initial sync, catch-up sync, and real-time event processing
 * from WebSocket messages.
 */
interface SyncManager {

    /**
     * Current sync state.
     */
    val state: StateFlow<SyncState>

    /**
     * Initialize sync on app startup or login.
     * Performs either a full sync (if no cursor) or catch-up sync (if cursor exists).
     * Returns true on success.
     */
    suspend fun initialize(): Boolean

    /**
     * Perform a full sync from scratch.
     * Fetches all user state and resets the cursor.
     * Returns true on success.
     */
    suspend fun fullSync(): Boolean

    /**
     * Catch up on events since the last known cursor.
     * Falls back to full sync if events have been pruned.
     * Returns true on success.
     */
    suspend fun catchUp(): Boolean

    /**
     * Handle a sync event received via WebSocket.
     * The event is a StoredEvent JSON object.
     */
    suspend fun handleSyncMessage(storedEvent: StoredEvent)

    /**
     * Clean up sync state (for logout).
     * Clears the cursor and resets state.
     */
    suspend fun cleanup()
}
