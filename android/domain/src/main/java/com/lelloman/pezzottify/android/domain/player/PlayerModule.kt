package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class PlayerModule {

    @Provides
    @Singleton
    fun providePlayer(
        staticsProvider: StaticsProvider,
        loggerFactory: LoggerFactory,
    ): Player = PlayerImpl(
        staticsProvider = staticsProvider,
        loggerFactory = loggerFactory,
    )
}