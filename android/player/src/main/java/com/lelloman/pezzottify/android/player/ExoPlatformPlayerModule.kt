package com.lelloman.pezzottify.android.player

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
@OptIn(UnstableApi::class)
internal class ExoPlatformPlayerModule {

    @Provides
    @Singleton
    fun providePlayerServiceEventsEmitter() = PlayerServiceEventsEmitter()

    @Provides
    @Singleton
    fun providePlatformPlayer(
        @ApplicationContext context: Context,
        playerServiceEventsEmitter: PlayerServiceEventsEmitter,
    ): PlatformPlayer = ExoPlatformPlayer(context, playerServiceEventsEmitter)
}