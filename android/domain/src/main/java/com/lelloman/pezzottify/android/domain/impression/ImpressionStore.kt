package com.lelloman.pezzottify.android.domain.impression

import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

interface ImpressionStore {

    suspend fun saveImpression(impression: Impression): Long

    suspend fun getPendingSyncImpressions(): List<Impression>

    suspend fun updateSyncStatus(id: Long, status: SyncStatus)

    suspend fun deleteImpression(id: Long)

    suspend fun deleteOldNonSyncedImpressions(olderThanMs: Long): Int

    suspend fun deleteSyncedImpressions(): Int

    suspend fun deleteAll()
}
