package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity
import kotlinx.coroutines.flow.Flow

@Dao
internal interface LikedContentDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(item: LikedContentEntity)

    @Query("SELECT * FROM ${LikedContentEntity.TABLE_NAME} WHERE ${LikedContentEntity.COLUMN_CONTENT_ID} = :contentId")
    fun getByContentId(contentId: String): Flow<LikedContentEntity?>

    @Query("SELECT * FROM ${LikedContentEntity.TABLE_NAME} WHERE ${LikedContentEntity.COLUMN_IS_LIKED} = 1")
    fun getAllLiked(): Flow<List<LikedContentEntity>>

    @Query("SELECT * FROM ${LikedContentEntity.TABLE_NAME} WHERE ${LikedContentEntity.COLUMN_IS_LIKED} = 1 AND ${LikedContentEntity.COLUMN_CONTENT_TYPE} IN (:types)")
    fun getLikedByTypes(types: List<String>): Flow<List<LikedContentEntity>>

    @Query("SELECT * FROM ${LikedContentEntity.TABLE_NAME} WHERE ${LikedContentEntity.COLUMN_SYNC_STATUS} IN ('PendingSync', 'SyncError')")
    fun getPendingSync(): Flow<List<LikedContentEntity>>

    @Query("UPDATE ${LikedContentEntity.TABLE_NAME} SET ${LikedContentEntity.COLUMN_SYNC_STATUS} = :status WHERE ${LikedContentEntity.COLUMN_CONTENT_ID} = :contentId")
    suspend fun updateSyncStatus(contentId: String, status: String)

    @Query("DELETE FROM ${LikedContentEntity.TABLE_NAME}")
    suspend fun deleteAll()

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(items: List<LikedContentEntity>)

    @Transaction
    suspend fun replaceAll(items: List<LikedContentEntity>) {
        deleteAll()
        insertAll(items)
    }
}
