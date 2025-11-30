package com.lelloman.pezzottify.android.localdata.internal

import android.content.Context
import androidx.room.Room
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

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .fallbackToDestructiveMigration(dropAllTables = true)
        .build()

    @Provides
    @Singleton
    internal fun provideUserDataDb(@ApplicationContext context: Context): UserDataDb = Room
        .databaseBuilder(context, UserDataDb::class.java, UserDataDb.NAME)
        .build()
}