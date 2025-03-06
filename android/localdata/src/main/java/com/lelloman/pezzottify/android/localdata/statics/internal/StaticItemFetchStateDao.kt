package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
internal interface StaticItemFetchStateDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(record: StaticItemFetchStateRecord)

    @Suppress("MaxLineLength")
    @Query("SELECT * FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_ITEM_ID} = :itemId")
    fun get(itemId: String): Flow<StaticItemFetchStateRecord?>

    @Suppress("MaxLineLength")
    @Query("DELETE FROM ${StaticItemFetchStateRecord.TABLE_NAME} WHERE ${StaticItemFetchStateRecord.COLUMN_ITEM_ID} = :itemId")
    fun delete(itemId: String)


}