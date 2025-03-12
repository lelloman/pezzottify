package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.Database
import androidx.room.RoomDatabase
import com.lelloman.pezzottify.android.localdata.internal.user.model.ViewedContent

@Database(entities = [ViewedContent::class], version = UserDataDb.VERSION)
internal abstract class UserDataDb : RoomDatabase() {

    abstract fun viewedContentDao(): ViewedContentDao

    companion object {
        const val VERSION = 1
        const val NAME = "user_data"
    }
}