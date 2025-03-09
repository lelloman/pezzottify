package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = Artist.TABLE_NAME)
data class Artist(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,
) : com.lelloman.pezzottify.android.domain.statics.Artist {

    companion object {
        const val TABLE_NAME = "artist"

        const val COLUMN_ID = "id"
    }
}

fun com.lelloman.pezzottify.android.domain.statics.Artist.quack(): Artist = Artist(
    id = id,
    name = name,
)