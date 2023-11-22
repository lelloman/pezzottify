package com.lelloman.pezzottify.android.domain

import android.content.Context
import com.lelloman.pezzottify.remoteapi.RemoteApi
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.Dispatchers
import java.io.File

@Module
@InstallIn(SingletonComponent::class)
class LoginModule {

    @Provides
    fun provideLoginManager(@ApplicationContext context: Context): LoginManager =
        LoginManagerImpl(
            RemoteApi.Factory.create("http://10.0.2.2:8080", Dispatchers.IO),
            File(context.filesDir, "2034hny")
        )
}