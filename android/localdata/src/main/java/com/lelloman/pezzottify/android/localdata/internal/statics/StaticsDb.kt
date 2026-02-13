package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.lelloman.pezzottify.android.localdata.internal.skeleton.SkeletonDao
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbum
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonMeta
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonTrack
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Album
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Track

@Database(
    entities = [
        Artist::class,
        Track::class,
        Album::class,
        StaticItemFetchStateRecord::class,
        // Skeleton entities
        SkeletonArtist::class,
        SkeletonAlbum::class,
        SkeletonAlbumArtist::class,
        SkeletonTrack::class,
        SkeletonMeta::class
    ],
    version = StaticsDb.VERSION,
    exportSchema = true
)
@TypeConverters(StaticsDbTypesConverter::class)
internal abstract class StaticsDb : RoomDatabase() {

    abstract fun staticsDao(): StaticsDao

    abstract fun staticItemFetchStateDao(): StaticItemFetchStateDao

    abstract fun skeletonDao(): SkeletonDao

    companion object {
        const val VERSION = 11
        const val NAME = "StaticsDb"
    }
}