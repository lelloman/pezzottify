package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = Artist.TABLE_NAME)
data class Artist(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    val id: String,

    val name: String,
) {
    companion object {
        const val TABLE_NAME = "artist"

        const val COLUMN_ID = "id"
    }
}