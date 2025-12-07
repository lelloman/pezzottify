package com.lelloman.pezzottify.android.localdata.internal.listening

import com.lelloman.pezzottify.android.domain.listening.ListeningEvent
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class ListeningEventStoreImpl @Inject constructor(
    private val dao: ListeningEventDao,
) : ListeningEventStore {

    override suspend fun saveEvent(event: ListeningEvent): Long {
        return dao.insert(event.toEntity())
    }

    override suspend fun updateEvent(event: ListeningEvent) {
        dao.update(event.toEntity())
    }

    override suspend fun getPendingSyncEvents(): List<ListeningEvent> =
        dao.getPendingSync().map { it.toDomain() }

    override suspend fun updateSyncStatus(id: Long, status: SyncStatus) {
        dao.updateSyncStatus(id, status.name)
    }

    override suspend fun getActiveSession(trackId: String): ListeningEvent? =
        dao.getActiveSession(trackId)?.toDomain()

    override suspend fun deleteEvent(id: Long) {
        dao.delete(id)
    }

    override suspend fun deleteOldNonSyncedEvents(olderThanMs: Long): Int =
        dao.deleteOldNonSynced(olderThanMs)

    override suspend fun deleteSyncedEvents(): Int =
        dao.deleteSynced()

    override suspend fun deleteAll() {
        dao.deleteAll()
    }

    private fun ListeningEvent.toEntity() = ListeningEventEntity(
        id = id,
        trackId = trackId,
        sessionId = sessionId,
        startedAt = startedAt,
        endedAt = endedAt,
        durationSeconds = durationSeconds,
        trackDurationSeconds = trackDurationSeconds,
        seekCount = seekCount,
        pauseCount = pauseCount,
        playbackContext = playbackContext.toStorageString(),
        syncStatus = syncStatus.name,
        createdAt = createdAt,
    )

    private fun ListeningEventEntity.toDomain() = ListeningEvent(
        id = id,
        trackId = trackId,
        sessionId = sessionId,
        startedAt = startedAt,
        endedAt = endedAt,
        durationSeconds = durationSeconds,
        trackDurationSeconds = trackDurationSeconds,
        seekCount = seekCount,
        pauseCount = pauseCount,
        playbackContext = playbackContext.toPlaybackContext(),
        syncStatus = SyncStatus.valueOf(syncStatus),
        createdAt = createdAt,
    )

    private fun PlaybackPlaylistContext.toStorageString(): String = when (this) {
        is PlaybackPlaylistContext.Album -> "Album:$albumId"
        is PlaybackPlaylistContext.UserPlaylist -> "UserPlaylist:$userPlaylistId:$isEdited"
        is PlaybackPlaylistContext.UserMix -> "UserMix"
    }

    private fun String.toPlaybackContext(): PlaybackPlaylistContext = when {
        startsWith("Album:") -> PlaybackPlaylistContext.Album(removePrefix("Album:"))
        startsWith("UserPlaylist:") -> {
            val parts = removePrefix("UserPlaylist:").split(":")
            PlaybackPlaylistContext.UserPlaylist(
                userPlaylistId = parts[0],
                isEdited = parts.getOrNull(1)?.toBooleanStrictOrNull() ?: false,
            )
        }
        else -> PlaybackPlaylistContext.UserMix
    }
}
