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
    val id: String,

    val name: String,

    val genre: List<String>,

    val portraitsImagesIds: List<String>,

    val related: List<String>,

    val portraitGroupImagesIds: List<String>,
) {
    companion object {
        const val TABLE_NAME = "Album"

        const val COLUMN_ID = "id"
    }
}