package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.internal.user.model.SearchHistoryEntryEntity
import kotlinx.coroutines.flow.Flow

@Dao
internal interface SearchHistoryEntryDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(entity: SearchHistoryEntryEntity)

    @Query(
        "SELECT * FROM ${SearchHistoryEntryEntity.TABLE_NAME} " +
                "ORDER BY created DESC " +
                "LIMIT :limit"
    )
    fun getRecent(limit: Int): Flow<List<SearchHistoryEntryEntity>>

    @Query("DELETE FROM ${SearchHistoryEntryEntity.TABLE_NAME}")
    suspend fun deleteAll()
}
