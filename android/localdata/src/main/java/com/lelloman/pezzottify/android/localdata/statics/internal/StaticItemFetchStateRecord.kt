package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = StaticItemFetchStateRecord.TABLE_NAME)
internal data class StaticItemFetchStateRecord(

    @PrimaryKey
    @ColumnInfo(name = COLUMN_ITEM_ID)
    val itemId: String,

    val loading: Boolean,

    val errorReason: String?,

    val lastUpdated: Long,

    val created: Long,
) {
    companion object {
        const val TABLE_NAME = "StaticItemFetchState"

        const val COLUMN_ITEM_ID = "itemId"
    }
}