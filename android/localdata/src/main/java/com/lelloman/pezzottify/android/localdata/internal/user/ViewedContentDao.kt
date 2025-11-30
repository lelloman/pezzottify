package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.internal.user.model.ViewedContent
import kotlinx.coroutines.flow.Flow

@Dao
internal interface ViewedContentDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(viewedContent: ViewedContent)

    @Query(
        "SELECT id, type, contentId, MAX(created) as created, synced FROM ${ViewedContent.TABLE_NAME} " +
                "WHERE ${ViewedContent.COLUMN_TYPE} IN (:allowedTypes) " +
                "GROUP BY ${ViewedContent.COLUMN_CONTENT_ID} " +
                "ORDER BY created DESC " +
                "LIMIT :limit;"
    )
    fun getRecentlyViewedContent(allowedTypes: List<String>, limit: Int): Flow<List<ViewedContent>>

    @Query("SELECT * FROM ${ViewedContent.TABLE_NAME} WHERE synced = 0;")
    fun getNotSynced(): Flow<List<ViewedContent>>

    @Query("DELETE FROM ${ViewedContent.TABLE_NAME};")
    suspend fun deleteAll()
}