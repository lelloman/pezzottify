package com.lelloman.pezzottify.android.localplayer.data

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "local_playlist")
data class LocalPlaylistEntity(
    @PrimaryKey
    val id: String,
    val name: String,
    val createdAt: Long
)
