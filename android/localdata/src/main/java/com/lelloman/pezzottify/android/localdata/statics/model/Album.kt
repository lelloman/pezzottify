package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

sealed interface ActivityPeriod {

    data class Timespan(

        val startYear: Int,

        val endYear: Int?,
    ) : ActivityPeriod

    data class Decade(
        val value: Int,
    ) : ActivityPeriod
}

@Entity
data class Album(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val genre: List<String>,

    override val portraitsImagesIds: List<String>,

    override val related: List<String>,

    override val portraitGroupImagesIds: List<String>,
) : com.lelloman.pezzottify.android.domain.statics.Album {
    companion object {
        const val TABLE_NAME = "album"

        const val COLUMN_ID = "id"
    }
}

fun com.lelloman.pezzottify.android.domain.statics.Album.quack(): Album = Album(
    id = id,
    name = name,
    genre = genre,
    portraitsImagesIds = portraitsImagesIds,
    related = related,
    portraitGroupImagesIds = portraitGroupImagesIds,
)