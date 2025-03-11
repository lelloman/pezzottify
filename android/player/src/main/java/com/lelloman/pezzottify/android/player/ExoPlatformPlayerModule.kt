package com.lelloman.pezzottify.android.player

import android.content.Context
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import com.lelloman.pezzottify.android.domain.auth.AuthStore
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
class ExoPlatformPlayerModule {

    @Provides
    @Singleton
    fun providePlatformPlayerFactory(
        @ApplicationContext context: Context,
        authStore: AuthStore
    ): PlatformPlayer.Factory =
        object : PlatformPlayer.Factory {
            override fun create(looper: Looper): PlatformPlayer = ExoPlatformPlayer(context, authStore, looper)
        }
}