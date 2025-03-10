package com.lelloman.pezzottify.android

import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class ApplicationModule {

    @Provides
    @Singleton
    fun provideLogLevelProvider(): StateFlow<@JvmWildcard LogLevel> = MutableStateFlow(LogLevel.Debug)

    @Provides
    @Singleton
    fun provideLoggerFactory(logLevelProvider: StateFlow<LogLevel>): LoggerFactory =
        LoggerFactory(logLevelProvider)


}