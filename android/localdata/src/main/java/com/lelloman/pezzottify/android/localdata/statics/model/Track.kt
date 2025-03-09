package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = Track.TABLE_NAME)
data class Track(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val albumId: String,
) : com.lelloman.pezzottify.android.domain.statics.Track {
    companion object {
        const val TABLE_NAME = "Track"

        const val COLUMN_ID = "id"
    }
}

fun com.lelloman.pezzottify.android.domain.statics.Track.quack(): Track = Track(
    id = id,
    name = name,
    albumId = albumId,
)