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

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .addMigrations(MIGRATION_1_2, MIGRATION_2_3)
        .build()

    @Provides
    @Singleton
    internal fun provideUserDataDb(@ApplicationContext context: Context): UserDataDb = Room
        .databaseBuilder(context, UserDataDb::class.java, UserDataDb.NAME)
        .build()
}