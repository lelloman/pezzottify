package com.lelloman.pezzottify.android.domain.listening

import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

interface ListeningEventStore {

    /** Saves event and returns the generated ID */
    suspend fun saveEvent(event: ListeningEvent): Long

    suspend fun updateEvent(event: ListeningEvent)

    suspend fun getPendingSyncEvents(): List<ListeningEvent>

    suspend fun updateSyncStatus(id: Long, status: SyncStatus)

    suspend fun getActiveSession(trackId: String): ListeningEvent?

    suspend fun deleteEvent(id: Long)

    suspend fun deleteOldNonSyncedEvents(olderThanMs: Long): Int

    suspend fun deleteAll()
}
