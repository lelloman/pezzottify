package com.lelloman.pezzottify.android.domain.app

import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class AppModule {

    @Provides
    @Singleton
    fun provideTimeProvider(): TimeProvider = TimeProvider { System.currentTimeMillis() }
}