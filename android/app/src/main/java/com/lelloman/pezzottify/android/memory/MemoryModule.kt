package com.lelloman.pezzottify.android.memory

import android.app.Application
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollector
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollectorImpl
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
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

    @Provides
    @Singleton
    fun provideStaticsCache(
        memoryPressureMonitor: MemoryPressureMonitor
    ): StaticsCache = StaticsCache(memoryPressureMonitor)

    @Provides
    @Singleton
    fun provideCacheMetricsCollector(): CacheMetricsCollector = CacheMetricsCollectorImpl()
}
