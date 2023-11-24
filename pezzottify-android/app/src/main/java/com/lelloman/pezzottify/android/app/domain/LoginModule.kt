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

@Module
@InstallIn(SingletonComponent::class)
class LoginModule {

    @Provides
    fun provideLoginManager(
        @ApplicationContext context: Context,
        loginOperations: Set<@JvmSuppressWildcards LoginOperation>,
        logoutOperations: Set<@JvmSuppressWildcards LogoutOperation>,
        remoteApi: RemoteApi,
    ): LoginManager = LoginManagerImpl(
        remoteApi = remoteApi,
        persistence = File(context.filesDir, "2034hny"),
        ioDispatcher = Dispatchers.IO,
        loginOperations = loginOperations,
        logoutOperations = logoutOperations,
    )
}