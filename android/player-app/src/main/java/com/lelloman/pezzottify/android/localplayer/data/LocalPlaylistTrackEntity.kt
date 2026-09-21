package com.lelloman.pezzottify.android.localplayer.data

import androidx.room.Entity
import androidx.room.ForeignKey

@Entity(
    tableName = "local_playlist_track",
    primaryKeys = ["playlistId", "position"],
    foreignKeys = [
        ForeignKey(
            entity = LocalPlaylistEntity::class,
            parentColumns = ["id"],
            childColumns = ["playlistId"],
            onDelete = ForeignKey.CASCADE
        )
    ]
)
data class LocalPlaylistTrackEntity(
    val playlistId: String,
    val position: Int,
    val uri: String,
    val displayName: String
)
