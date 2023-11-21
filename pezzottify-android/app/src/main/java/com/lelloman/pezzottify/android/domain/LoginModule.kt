package com.lelloman.pezzottify.android.domain

import com.lelloman.pezzottify.remoteapi.RemoteApi
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.Dispatchers

@Module
@InstallIn(SingletonComponent::class)
class LoginModule {

    @Provides
    fun provideLoginManager(): LoginManager =
        MockLoginManager(RemoteApi.Factory.create("http://10.0.2.2:8080", Dispatchers.IO))
}