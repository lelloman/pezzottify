package com.lelloman.pezzottify.android.localdata.internal

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDb
import com.lelloman.pezzottify.android.localdata.internal.user.UserLocalDataDb
import com.lelloman.pezzottify.android.localdata.internal.usercontent.UserContentDb
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
internal class DbModule {

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .fallbackToDestructiveMigration(dropAllTables = true)
        .build()

    @Provides
    @Singleton
    internal fun provideUserLocalDataDb(@ApplicationContext context: Context): UserLocalDataDb = Room
        .databaseBuilder(context, UserLocalDataDb::class.java, UserLocalDataDb.NAME)
        .addMigrations(UserLocalDataDb.MIGRATION_1_2)
        .build()

    @Provides
    @Singleton
    internal fun provideUserContentDb(@ApplicationContext context: Context): UserContentDb = Room
        .databaseBuilder(context, UserContentDb::class.java, UserContentDb.NAME)
        .addMigrations(
            UserContentDb.MIGRATION_1_2,
            UserContentDb.MIGRATION_2_3,
            UserContentDb.MIGRATION_3_4,
            UserContentDb.MIGRATION_4_5,
            UserContentDb.MIGRATION_5_6,
        )
        .build()
}