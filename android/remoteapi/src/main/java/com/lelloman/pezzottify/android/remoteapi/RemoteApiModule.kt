package com.lelloman.pezzottify.android.remoteapi

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.remoteapi.internal.RemoteApiClientImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class RemoteApiModule {

    @Provides
    @Singleton
    fun provideRemoteApiClient(
        authStore: AuthStore,
        configStore: ConfigStore
    ): RemoteApiClient = RemoteApiClientImpl(
        okhttpClientBuilder = OkHttpClient.Builder(),
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