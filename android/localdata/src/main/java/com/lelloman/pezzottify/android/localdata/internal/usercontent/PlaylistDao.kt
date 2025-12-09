package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.PlaylistEntity
import kotlinx.coroutines.flow.Flow

@Dao
internal interface PlaylistDao {

    @Query("SELECT * FROM ${PlaylistEntity.TABLE_NAME}")
    fun getAll(): Flow<List<PlaylistEntity>>

    @Query("SELECT * FROM ${PlaylistEntity.TABLE_NAME} WHERE ${PlaylistEntity.COLUMN_ID} = :playlistId")
    fun getById(playlistId: String): Flow<PlaylistEntity?>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(item: PlaylistEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(items: List<PlaylistEntity>)

    @Query("DELETE FROM ${PlaylistEntity.TABLE_NAME} WHERE ${PlaylistEntity.COLUMN_ID} = :playlistId")
    suspend fun deleteById(playlistId: String)

    @Query("UPDATE ${PlaylistEntity.TABLE_NAME} SET ${PlaylistEntity.COLUMN_NAME} = :name WHERE ${PlaylistEntity.COLUMN_ID} = :playlistId")
    suspend fun updateName(playlistId: String, name: String)

    @Query("UPDATE ${PlaylistEntity.TABLE_NAME} SET ${PlaylistEntity.COLUMN_TRACK_IDS} = :trackIds WHERE ${PlaylistEntity.COLUMN_ID} = :playlistId")
    suspend fun updateTrackIds(playlistId: String, trackIds: List<String>)

    @Query("DELETE FROM ${PlaylistEntity.TABLE_NAME}")
    suspend fun deleteAll()

    @Transaction
    suspend fun replaceAll(items: List<PlaylistEntity>) {
        deleteAll()
        insertAll(items)
    }
}
