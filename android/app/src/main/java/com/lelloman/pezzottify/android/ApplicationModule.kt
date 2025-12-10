package com.lelloman.pezzottify.android

import android.content.Context
import coil3.ImageLoader
import coil3.intercept.Interceptor
import coil3.network.httpHeaders
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.ImageResult
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.logging.LogFileManager
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import java.net.URI
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class ApplicationModule {

    @Provides
    @Singleton
    fun provideApplicationScope(): CoroutineScope =
        CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @Provides
    @Singleton
    fun provideLogLevelProvider(): StateFlow<@JvmWildcard LogLevel> =
        MutableStateFlow(LogLevel.Debug)

    @Provides
    @Singleton
    fun provideLogFileManager(
        @ApplicationContext context: Context
    ): LogFileManager = LogFileManager(context)

    @Provides
    @Singleton
    fun provideLoggerFactory(
        logLevelProvider: StateFlow<LogLevel>,
        userSettingsStore: UserSettingsStore,
        logFileManager: LogFileManager,
    ): LoggerFactory = LoggerFactory(
        logLevelProvider = logLevelProvider,
        fileLoggingEnabled = userSettingsStore.isFileLoggingEnabled,
        logDir = logFileManager.logDir,
    )

    @Provides
    @Singleton
    fun provideImageLoader(
        @ApplicationContext context: Context,
        authStore: AuthStore,
        configStore: ConfigStore,
        okHttpClientFactory: OkHttpClientFactory,
    ): ImageLoader {
        // Create OkHttpClient with SSL pinning for our server
        val pinnedOkHttpClient = okHttpClientFactory
            .createBuilder(configStore.baseUrl.value)
            .build()

        // Create a plain OkHttpClient for external URLs (no SSL pinning, no auth)
        val externalOkHttpClient = OkHttpClient.Builder().build()

        // Create a routing call factory that uses the appropriate client based on URL
        val routingCallFactory = RoutingCallFactory(
            baseUrl = configStore.baseUrl.value,
            internalClient = pinnedOkHttpClient,
            externalClient = externalOkHttpClient,
        )

        return ImageLoader.Builder(context)
            .components {
                add(CoilAuthTokenInterceptor(authStore, configStore))
                add(OkHttpNetworkFetcherFactory(callFactory = { routingCallFactory }))
            }
            .build()
    }

    @Provides
    @Singleton
    fun provideBuildInfo(): BuildInfo = object : BuildInfo {
        override val buildVariant: String = BuildConfig.BUILD_TYPE
        override val versionName: String = BuildConfig.VERSION_NAME
        override val gitCommit: String = BuildConfig.GIT_COMMIT
    }
}

/**
 * Coil interceptor that adds auth token only to requests going to our server.
 * External URLs (e.g., album cover images from music providers) are not modified.
 */
private class CoilAuthTokenInterceptor(
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
) : Interceptor {

    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
        val requestUrl = chain.request.data.toString()

        // Only add auth header for requests to our server
        if (!isInternalUrl(requestUrl)) {
            return chain.proceed()
        }

        val authToken =
            (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken
        if (authToken == null || authToken.isEmpty()) {
            return chain.proceed()
        }

        val oldRequest = chain.request
        val oldHeaders = oldRequest.httpHeaders
        val newHeaders = oldHeaders.newBuilder().add("Authorization", "$authToken").build()
        val newRequestBuilder = chain.request.newBuilder()
            .httpHeaders(newHeaders)
        return chain.withRequest(newRequestBuilder.build()).proceed()
    }

    private fun isInternalUrl(url: String): Boolean {
        val baseHost = extractHost(configStore.baseUrl.value) ?: return false
        val requestHost = extractHost(url) ?: return false
        return requestHost == baseHost
    }

    private fun extractHost(url: String): String? {
        return try {
            URI(url).host
        } catch (_: Exception) {
            null
        }
    }
}

/**
 * OkHttp Call.Factory that routes requests to different OkHttpClients based on URL host.
 * Internal URLs (to our server) use the pinned client, external URLs use a plain client.
 */
private class RoutingCallFactory(
    private val baseUrl: String,
    private val internalClient: OkHttpClient,
    private val externalClient: OkHttpClient,
) : okhttp3.Call.Factory {

    private val internalHost = extractHost(baseUrl)

    override fun newCall(request: okhttp3.Request): okhttp3.Call {
        val requestHost = extractHost(request.url.toString())
        val client = if (requestHost == internalHost) {
            internalClient
        } else {
            externalClient
        }
        return client.newCall(request)
    }

    private fun extractHost(url: String): String? {
        return try {
            URI(url).host
        } catch (_: Exception) {
            null
        }
    }
}
