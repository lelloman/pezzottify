package com.lelloman.pezzottify.android.localdata.internal.listening

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update

@Dao
internal interface ListeningEventDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(event: ListeningEventEntity): Long

    @Update
    suspend fun update(event: ListeningEventEntity)

    @Query("SELECT * FROM listening_event WHERE sync_status = 'PendingSync' ORDER BY created_at ASC")
    suspend fun getPendingSync(): List<ListeningEventEntity>

    @Query("UPDATE listening_event SET sync_status = :status WHERE id = :id")
    suspend fun updateSyncStatus(id: Long, status: String)

    @Query("SELECT * FROM listening_event WHERE track_id = :trackId AND ended_at IS NULL LIMIT 1")
    suspend fun getActiveSession(trackId: String): ListeningEventEntity?

    @Query("DELETE FROM listening_event WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM listening_event WHERE sync_status != 'Synced' AND created_at < :olderThanMs")
    suspend fun deleteOldNonSynced(olderThanMs: Long): Int

    @Query("DELETE FROM listening_event WHERE sync_status = 'Synced'")
    suspend fun deleteSynced(): Int

    @Query("DELETE FROM listening_event")
    suspend fun deleteAll()
}
