package com.lelloman.pezzottify.android.storage

import android.app.Application
import com.lelloman.pezzottify.android.domain.storage.StorageMonitor
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class StorageModule {

    @Provides
    @Singleton
    fun provideStorageMonitor(
        application: Application
    ): StorageMonitor = AndroidStorageMonitor(application)
}
