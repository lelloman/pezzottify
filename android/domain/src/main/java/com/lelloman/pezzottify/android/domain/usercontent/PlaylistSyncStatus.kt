package com.lelloman.pezzottify.android.domain.usercontent

enum class PlaylistSyncStatus {
    /** Playlist is in sync with the server */
    Synced,
    /** Playlist was created locally and needs to be uploaded to the server */
    PendingCreate,
    /** Playlist was modified locally and needs to be updated on the server */
    PendingUpdate,
    /** Playlist was deleted locally and needs to be deleted on the server */
    PendingDelete,
    /** Currently syncing with the server */
    Syncing,
    /** Failed to sync with the server */
    SyncError,
}
