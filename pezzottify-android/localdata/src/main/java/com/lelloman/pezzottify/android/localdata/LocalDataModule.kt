package com.lelloman.pezzottify.android.localdata

import android.content.Context
import androidx.room.Room
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent

@Module
@InstallIn(SingletonComponent::class)
class Module {

    @Provides
    fun provideLocalDb(@ApplicationContext context: Context) =
        Room.databaseBuilder(context, LocalDb::class.java, LocalDb.DB_NAME).build()

    @Provides
    fun provideStaticsDao(localDb: LocalDb): StaticsDao = localDb.staticsDao()

}