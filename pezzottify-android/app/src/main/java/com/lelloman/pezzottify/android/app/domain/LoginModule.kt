package com.lelloman.pezzottify.android.app.domain

import android.content.Context
import com.lelloman.pezzottify.remoteapi.RemoteApi
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.Dispatchers
import java.io.File
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class LoginModule {

    @Provides
    @Singleton
    fun provideLoginManager(
        @ApplicationContext context: Context,
        remoteApi: RemoteApi,
    ): LoginManager = LoginManagerImpl(
        remoteApi = remoteApi,
        persistence = File(context.filesDir, "2034hny"),
        ioDispatcher = Dispatchers.IO,
    )

    @Provides
    @Singleton
    fun provideLoginStateOperationsCollector(
        loginOperations: Set<@JvmSuppressWildcards LoginOperation>,
        logoutOperations: Set<@JvmSuppressWildcards LogoutOperation>,
    ) = LoginStateOperationsCollector(loginOperations, logoutOperations)
}