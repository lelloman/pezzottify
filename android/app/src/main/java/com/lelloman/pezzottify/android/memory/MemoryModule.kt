package com.lelloman.pezzottify.android.memory

import android.app.Application
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class MemoryModule {

    @Provides
    @Singleton
    fun provideMemoryPressureMonitor(
        application: Application
    ): MemoryPressureMonitor = AndroidMemoryPressureMonitor(application)
}
