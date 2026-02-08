package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.internal.CompositePlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.internal.PlaybackMetadataProviderImpl
import com.lelloman.pezzottify.android.domain.player.internal.PlaybackRouter
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.player.internal.RemotePlaybackController
import com.lelloman.pezzottify.android.domain.player.internal.RemotePlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
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
    fun providePlayerImpl(
        staticsProvider: StaticsProvider,
        loggerFactory: LoggerFactory,
        platformPlayer: PlatformPlayer,
        configStore: ConfigStore,
        userPlaylistStore: UserPlaylistStore,
        playbackStateStore: PlaybackStateStore,
    ): PlayerImpl = PlayerImpl(
        staticsProvider = staticsProvider,
        loggerFactory = loggerFactory,
        platformPlayer = platformPlayer,
        configStore = configStore,
        userPlaylistStore = userPlaylistStore,
        playbackStateStore = playbackStateStore,
    )

    @Provides
    @Singleton
    fun providePlaybackMetadataProviderImpl(
        player: PlayerImpl,
        platformPlayer: PlatformPlayer,
        staticsProvider: StaticsProvider,
        configStore: ConfigStore,
        loggerFactory: LoggerFactory,
    ): PlaybackMetadataProviderImpl = PlaybackMetadataProviderImpl(
        player = player,
        platformPlayer = platformPlayer,
        staticsProvider = staticsProvider,
        configStore = configStore,
        loggerFactory = loggerFactory,
    )

    @Provides
    @Singleton
    fun providePlayer(
        playerImpl: PlayerImpl,
        remotePlaybackController: RemotePlaybackController,
        playbackModeManager: PlaybackModeManager,
        playbackSessionHandler: PlaybackSessionHandler,
        loggerFactory: LoggerFactory,
    ): PezzottifyPlayer = PlaybackRouter(
        localPlayer = playerImpl,
        remoteController = remotePlaybackController,
        playbackModeManager = playbackModeManager,
        playbackSessionHandler = playbackSessionHandler,
        loggerFactory = loggerFactory,
    )

    @Provides
    @Singleton
    fun providePlaybackMetadataProvider(
        localProvider: PlaybackMetadataProviderImpl,
        remoteProvider: RemotePlaybackMetadataProvider,
        playbackModeManager: PlaybackModeManager,
    ): PlaybackMetadataProvider = CompositePlaybackMetadataProvider(
        localProvider = localProvider,
        remoteProvider = remoteProvider,
        playbackModeManager = playbackModeManager,
    )
}
