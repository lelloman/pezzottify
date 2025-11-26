package com.lelloman.pezzottify.android.localdata.internal.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.statics.Image

@Entity(tableName = Artist.TABLE_NAME)
internal data class Artist(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val portraits: List<Image>,

    override val portraitGroup: List<Image>,
) : com.lelloman.pezzottify.android.domain.statics.Artist {

    companion object {
        const val TABLE_NAME = "artist"

        const val COLUMN_ID = "id"
    }
}

internal fun com.lelloman.pezzottify.android.domain.statics.Artist.quack(): Artist = Artist(
    id = id,
    name = name,
    portraits = portraits,
    portraitGroup = portraitGroup,
)