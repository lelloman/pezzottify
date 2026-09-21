package com.lelloman.pezzottify.android.localplayer.data

import androidx.room.Database
import androidx.room.RoomDatabase

@Database(
    entities = [
        LocalPlaylistEntity::class,
        LocalPlaylistTrackEntity::class
    ],
    version = 1,
    exportSchema = false
)
abstract class LocalPlayerDb : RoomDatabase() {

    abstract fun playlistDao(): LocalPlaylistDao

    companion object {
        const val NAME = "local_player"
    }
}
