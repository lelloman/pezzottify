package com.lelloman.pezzottify.android.remoteapi

import com.lelloman.pezzottify.android.localdata.auth.AuthState
import com.lelloman.pezzottify.android.localdata.auth.AuthStore
import com.lelloman.pezzottify.android.localdata.config.ConfigStore
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class RemoteApiModule {

    @Provides
    @Singleton
    fun provideRemoteApiClient(
        authStore: AuthStore,
        configStore: ConfigStore
    ): RemoteApiClient = RemoteApiClient.Factory
        .create(
            hostUrlProvider = object : RemoteApiClient.HostUrlProvider {
                override val hostUrl: StateFlow<String>
                    get() = configStore.baseUrl

            },
            credentialsProvider = object : RemoteApiCredentialsProvider {
                override val authToken: String
                    get() = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken ?: ""
            }
        )
}