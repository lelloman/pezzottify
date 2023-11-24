package com.lelloman.pezzottify.android.localdata.model

import androidx.room.Entity
import androidx.room.PrimaryKey

interface Playlist {
    val id: String
    val audioTracksIds: List<String>
    val name: String
}

@Entity(tableName = Album.TABLE_NAME)
data class Album(
    @PrimaryKey
    override val id: String = "",

    override val name: String,

    override val audioTracksIds: List<String> = emptyList(),

    val coverImageId: String? = null,

    val sideImagesIds: List<String> = emptyList(),

    val artistsIds: List<String>,
) : Playlist {
    companion object {
        const val TABLE_NAME = "album"
    }
}

data class UserPlayList(
    override val id: String,

    override val name: String,

    override val audioTracksIds: List<String>,
) : Playlist

class Albums : ArrayList<Album>()