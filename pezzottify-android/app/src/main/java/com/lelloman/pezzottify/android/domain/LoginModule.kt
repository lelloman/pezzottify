package com.lelloman.pezzottify.android.domain

import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@Module
@InstallIn(SingletonComponent::class)
class LoginModule {

    @Provides
    fun provideLoginManager(): LoginManager {
        return MockLoginManager(object : RemoteLoginInteractor {
            override suspend fun doLogin() {
                TODO("Not yet implemented")
            }
        })
    }
}