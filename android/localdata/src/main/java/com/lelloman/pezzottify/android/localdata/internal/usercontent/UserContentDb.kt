package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity

@Database(
    entities = [LikedContentEntity::class],
    version = UserContentDb.VERSION,
)
@TypeConverters(UserContentTypeConverters::class)
internal abstract class UserContentDb : RoomDatabase() {

    abstract fun likedContentDao(): LikedContentDao

    companion object {
        const val VERSION = 1
        const val NAME = "user_content"
    }
}
