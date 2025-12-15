package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.internal.PlaybackMetadataProviderImpl
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
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
        platformPlayer: PlatformPlayer,
        configStore: ConfigStore,
        userPlaylistStore: UserPlaylistStore,
    ): PezzottifyPlayer = PlayerImpl(
        staticsProvider = staticsProvider,
        loggerFactory = loggerFactory,
        platformPlayer = platformPlayer,
        configStore = configStore,
        userPlaylistStore = userPlaylistStore,
    )

    @Provides
    @Singleton
    fun providePlaybackMetadataProvider(
        player: PezzottifyPlayer,
        platformPlayer: PlatformPlayer,
        staticsProvider: StaticsProvider,
        configStore: ConfigStore,
        loggerFactory: LoggerFactory,
    ): PlaybackMetadataProvider = PlaybackMetadataProviderImpl(
        player = player,
        platformPlayer = platformPlayer,
        staticsProvider = staticsProvider,
        configStore = configStore,
        loggerFactory = loggerFactory,
    )
}