package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = Track.TABLE_NAME)
data class Track(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    val id: String,

    val name: String,

    val albumId: String,
) {
    companion object {
        const val TABLE_NAME = "Track"

        const val COLUMN_ID = "id"
    }
}