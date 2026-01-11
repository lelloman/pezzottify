package com.lelloman.pezzottify.android.localplayer

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.localplayer.data.LocalPlayerDb
import com.lelloman.pezzottify.android.localplayer.data.LocalPlaylistDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object LocalPlayerModule {

    @Provides
    @Singleton
    fun provideLocalPlayerDb(@ApplicationContext context: Context): LocalPlayerDb =
        Room.databaseBuilder(context, LocalPlayerDb::class.java, LocalPlayerDb.NAME)
            .fallbackToDestructiveMigration(dropAllTables = true)
            .build()

    @Provides
    fun provideLocalPlaylistDao(db: LocalPlayerDb): LocalPlaylistDao =
        db.playlistDao()
}
