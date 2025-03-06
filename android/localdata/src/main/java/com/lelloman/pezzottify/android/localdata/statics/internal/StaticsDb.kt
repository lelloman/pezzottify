package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.lelloman.pezzottify.android.localdata.statics.model.Album
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.statics.model.ArtistDiscography
import com.lelloman.pezzottify.android.localdata.statics.model.Track

@Database(
    entities = [
        Artist::class,
        Track::class,
        Album::class,
        StaticItemFetchStateRecord::class,
        ArtistDiscography::class
    ],
    version = StaticsDb.VERSION,
    exportSchema = true
)
@TypeConverters(StaticsDbTypesConverter::class)
internal abstract class StaticsDb : RoomDatabase() {

    abstract fun staticsDao(): StaticsDao

    abstract fun staticItemFetchStateDao(): StaticItemFetchStateDao

    companion object {
        const val VERSION = 1
    }
}