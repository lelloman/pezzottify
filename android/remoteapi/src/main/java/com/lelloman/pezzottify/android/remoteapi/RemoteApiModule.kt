package com.lelloman.pezzottify.android.remoteapi

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.remoteapi.internal.HttpLoggingInterceptor
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import com.lelloman.pezzottify.android.remoteapi.internal.RemoteApiClientImpl
import com.lelloman.pezzottify.android.remoteapi.internal.SessionExpiredInterceptor
import com.lelloman.pezzottify.android.remoteapi.internal.websocket.WebSocketManagerImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Qualifier
import javax.inject.Singleton

@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class WebSocketScope

@InstallIn(SingletonComponent::class)
@Module
class RemoteApiModule {

    @Provides
    @Singleton
    @WebSocketScope
    fun provideWebSocketCoroutineScope(): CoroutineScope =
        CoroutineScope(SupervisorJob() + Dispatchers.IO)

    @Provides
    @Singleton
    fun provideWebSocketManager(
        authStore: AuthStore,
        configStore: ConfigStore,
        okHttpClientFactory: OkHttpClientFactory,
        @WebSocketScope coroutineScope: CoroutineScope,
        loggerFactory: LoggerFactory,
    ): WebSocketManager = WebSocketManagerImpl(
        authStore = authStore,
        configStore = configStore,
        okHttpClientFactory = okHttpClientFactory,
        coroutineScope = coroutineScope,
        loggerFactory = loggerFactory,
    )

    @Provides
    @Singleton
    fun provideRemoteApiClient(
        authStore: AuthStore,
        configStore: ConfigStore,
        okHttpClientFactory: OkHttpClientFactory,
        sessionExpiredHandler: SessionExpiredHandler,
        tokenRefresher: TokenRefresher,
        loggerFactory: LoggerFactory,
    ): RemoteApiClient {
        val httpLogger = loggerFactory.getLogger("HTTP")
        val sessionLogger = loggerFactory.getLogger("SessionExpired")
        return RemoteApiClientImpl(
            okHttpClientFactory = okHttpClientFactory,
            hostUrlProvider = object : RemoteApiClient.HostUrlProvider {
                override val hostUrl: StateFlow<String>
                    get() = configStore.baseUrl

            },
            credentialsProvider = object : RemoteApiCredentialsProvider {
                override val authToken: String
                    get() = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken ?: ""
            },
            interceptors = listOf(
                SessionExpiredInterceptor(sessionExpiredHandler, tokenRefresher, sessionLogger),
                HttpLoggingInterceptor(httpLogger),
            ),
        )
    }
}