package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
internal interface StaticItemFetchStateDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(record: StaticItemFetchStateRecord): Long

    @Suppress("MaxLineLength")
    @Query("SELECT * FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_ITEM_ID} = :itemId")
    fun get(itemId: String): Flow<StaticItemFetchStateRecord?>

    @Query("SELECT * FROM ${StaticItemFetchStateRecord.TABLE_NAME}")
    fun getAll(): Flow<List<StaticItemFetchStateRecord>>

    @Query("SELECT COUNT(*) FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_LOADING} = 1")
    suspend fun getLoadingItemsCount(): Int

    @Query("UPDATE ${StaticItemFetchStateRecord.TABLE_NAME} SET ${StaticItemFetchStateRecord.COLUMN_LOADING} = 0")
    suspend fun resetLoadingStates()

    @Suppress("MaxLineLength")
    @Query("SELECT * FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_LOADING} = 0 AND (${StaticItemFetchStateRecord.COLUMN_TRY_NEXT_TIME} IS NULL OR ${StaticItemFetchStateRecord.COLUMN_TRY_NEXT_TIME} <= :currentTime)")
    suspend fun getAllIdle(currentTime: Long): List<StaticItemFetchStateRecord>

    @Suppress("MaxLineLength")
    @Query("DELETE FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_ITEM_ID} = :itemId")
    fun delete(itemId: String): Int
}