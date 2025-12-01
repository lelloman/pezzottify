package com.lelloman.pezzottify.android.domain.listening

/**
 * Domain DTO for syncing listening events to the server.
 * This keeps the domain layer independent of remoteapi models.
 */
data class ListeningEventSyncData(
    val trackId: String,
    val sessionId: String,
    val startedAt: Long,        // Unix timestamp seconds
    val endedAt: Long?,         // Unix timestamp seconds
    val durationSeconds: Int,
    val trackDurationSeconds: Int,
    val seekCount: Int,
    val pauseCount: Int,
    val playbackContext: String,
)
