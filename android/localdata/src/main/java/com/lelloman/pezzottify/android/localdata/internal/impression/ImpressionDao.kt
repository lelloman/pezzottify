package com.lelloman.pezzottify.android.localdata.internal.impression

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query

@Dao
internal interface ImpressionDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(impression: ImpressionEntity): Long

    @Query("SELECT * FROM impression WHERE sync_status = 'PendingSync' ORDER BY created_at ASC")
    suspend fun getPendingSync(): List<ImpressionEntity>

    @Query("UPDATE impression SET sync_status = :status WHERE id = :id")
    suspend fun updateSyncStatus(id: Long, status: String)

    @Query("DELETE FROM impression WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM impression WHERE sync_status != 'Synced' AND created_at < :olderThanMs")
    suspend fun deleteOldNonSynced(olderThanMs: Long): Int

    @Query("DELETE FROM impression WHERE sync_status = 'Synced'")
    suspend fun deleteSynced(): Int

    @Query("DELETE FROM impression")
    suspend fun deleteAll()
}
