package com.lelloman.pezzottify.android.domain.listening

import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

data class ListeningEvent(
    val id: Long = 0,
    val trackId: String,
    val sessionId: String,
    val startedAt: Long,            // Unix timestamp ms
    val endedAt: Long?,             // Unix timestamp ms, null if still playing
    val durationSeconds: Int,       // Actual listening time (excluding pauses)
    val trackDurationSeconds: Int,
    val seekCount: Int = 0,
    val pauseCount: Int = 0,
    val playbackContext: PlaybackPlaylistContext,
    val syncStatus: SyncStatus = SyncStatus.PendingSync,
    val createdAt: Long,
)

fun ListeningEvent.toSyncData() = ListeningEventSyncData(
    trackId = trackId,
    sessionId = sessionId,
    startedAt = startedAt / 1000,  // Convert ms to Unix seconds
    endedAt = endedAt?.let { it / 1000 },
    durationSeconds = durationSeconds,
    trackDurationSeconds = trackDurationSeconds,
    seekCount = seekCount,
    pauseCount = pauseCount,
    playbackContext = playbackContext.toApiString(),
)

fun PlaybackPlaylistContext.toApiString(): String = when (this) {
    is PlaybackPlaylistContext.Album -> "album"
    is PlaybackPlaylistContext.UserPlaylist -> if (isEdited) "queue" else "playlist"
    is PlaybackPlaylistContext.UserMix -> "queue"
}
