package com.lelloman.pezzottify.android.localdata.internal

import android.content.Context
import androidx.room.Room
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDb
import com.lelloman.pezzottify.android.localdata.internal.user.UserDataDb
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
internal class DbModule {

    private val MIGRATION_1_2 = object : Migration(1, 2) {
        override fun migrate(db: SupportSQLiteDatabase) {
            // Add date column to Album table with default value of 0
            db.execSQL("ALTER TABLE Album ADD COLUMN date INTEGER NOT NULL DEFAULT 0")
        }
    }

    private val MIGRATION_2_3 = object : Migration(2, 3) {
        override fun migrate(db: SupportSQLiteDatabase) {
            // Add timestamp columns to static_item_fetch_state table
            db.execSQL("ALTER TABLE static_item_fetch_state ADD COLUMN lastAttemptTime INTEGER")
            db.execSQL("ALTER TABLE static_item_fetch_state ADD COLUMN tryNextTime INTEGER")
        }
    }

    private val MIGRATION_3_4 = object : Migration(3, 4) {
        override fun migrate(db: SupportSQLiteDatabase) {
            // Add image columns to Artist table (portraits and portraitGroup)
            // Using empty JSON array as default value
            db.execSQL("ALTER TABLE artist ADD COLUMN portraits TEXT NOT NULL DEFAULT '[]'")
            db.execSQL("ALTER TABLE artist ADD COLUMN portraitGroup TEXT NOT NULL DEFAULT '[]'")

            // Add image columns to Album table (covers and coverGroup were likely missing too)
            // Check if they exist first, if not add them
            db.execSQL("ALTER TABLE album ADD COLUMN covers TEXT NOT NULL DEFAULT '[]'")
            db.execSQL("ALTER TABLE album ADD COLUMN coverGroup TEXT NOT NULL DEFAULT '[]'")

            // Clear cached data to force re-sync from server with proper image data
            db.execSQL("DELETE FROM artist")
            db.execSQL("DELETE FROM album")
            db.execSQL("DELETE FROM static_item_fetch_state")
        }
    }

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .addMigrations(MIGRATION_1_2, MIGRATION_2_3, MIGRATION_3_4)
        .build()

    @Provides
    @Singleton
    internal fun provideUserDataDb(@ApplicationContext context: Context): UserDataDb = Room
        .databaseBuilder(context, UserDataDb::class.java, UserDataDb.NAME)
        .build()
}