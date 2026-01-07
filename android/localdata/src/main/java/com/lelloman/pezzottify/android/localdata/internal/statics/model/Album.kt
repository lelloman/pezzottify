package com.lelloman.pezzottify.android.localdata.internal.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import kotlinx.serialization.Serializable

@Serializable
data class Disc(
    override val tracksIds: List<String>,
) : com.lelloman.pezzottify.android.domain.statics.Disc

@Entity
internal data class Album(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val date: Int,

    override val displayImageId: String?,

    override val artistsIds: List<String>,

    override val discs: List<Disc>,

    ) : com.lelloman.pezzottify.android.domain.statics.Album {

    companion object {
        const val TABLE_NAME = "album"

        const val COLUMN_ID = "id"
    }
}

internal fun com.lelloman.pezzottify.android.domain.statics.Album.quack(): Album = Album(
    id = id,
    name = name,
    date = date,
    displayImageId = displayImageId,
    artistsIds = artistsIds,
    discs = discs.map { Disc(tracksIds = it.tracksIds) },
)