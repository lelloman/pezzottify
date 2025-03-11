package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import javax.inject.Singleton

@InstallIn(Singleton::class)
@Module
class PlayerModule {

    @Provides
    @Singleton
    fun providePlayer(): Player = PlayerImpl()
}