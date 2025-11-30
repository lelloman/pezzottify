package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.lelloman.pezzottify.android.localdata.internal.user.model.SearchHistoryEntryEntity
import com.lelloman.pezzottify.android.localdata.internal.user.model.ViewedContent

@Database(
    entities = [ViewedContent::class, SearchHistoryEntryEntity::class],
    version = UserDataDb.VERSION
)
@TypeConverters(UserDataTypeConverters::class)
internal abstract class UserDataDb : RoomDatabase() {

    abstract fun viewedContentDao(): ViewedContentDao

    abstract fun searchHistoryEntryDao(): SearchHistoryEntryDao

    companion object {
        const val VERSION = 2
        const val NAME = "user_data"

        val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS ${SearchHistoryEntryEntity.TABLE_NAME} (
                        id TEXT NOT NULL PRIMARY KEY,
                        query TEXT NOT NULL,
                        contentType TEXT NOT NULL,
                        contentId TEXT NOT NULL,
                        created INTEGER NOT NULL
                    )
                    """.trimIndent()
                )
            }
        }
    }
}