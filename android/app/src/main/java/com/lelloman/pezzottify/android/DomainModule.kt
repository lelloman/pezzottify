package com.lelloman.pezzottify.android

import com.lelloman.pezzottify.android.localdata.DefaultHostUrl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@Module
@InstallIn(SingletonComponent::class)
class DomainModule {

    @Provides
    @DefaultHostUrl
    fun provideDefaultHostUrl() = "http://10.0.2.2:3001"
}