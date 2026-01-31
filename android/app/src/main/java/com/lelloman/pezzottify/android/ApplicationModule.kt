package com.lelloman.pezzottify.android

import android.content.Context
import coil3.ImageLoader
import coil3.intercept.Interceptor as CoilInterceptor
import coil3.network.httpHeaders
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.ImageResult
import com.lelloman.pezzottify.android.cache.CacheManagerImpl
import com.lelloman.pezzottify.android.cache.CoilImageCacheManager
import com.lelloman.pezzottify.android.cache.TrackingDiskCache
import com.lelloman.pezzottify.android.domain.cache.CacheManager
import com.lelloman.pezzottify.android.domain.cache.ImageCacheManager
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import okio.Path.Companion.toOkioPath
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
import okhttp3.Interceptor as OkHttpInterceptor
import okhttp3.OkHttpClient
import okhttp3.Response
import java.io.IOException
import java.net.URI
import javax.inject.Singleton
import kotlin.math.min
import kotlin.math.pow

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
    fun provideTrackingDiskCache(
        @ApplicationContext context: Context,
    ): TrackingDiskCache {
        return TrackingDiskCache.create(
            directory = context.cacheDir.resolve("image_cache").toOkioPath(),
            maxSizeBytes = 50L * 1024 * 1024, // 50 MB
        )
    }

    @Provides
    @Singleton
    fun provideImageLoader(
        @ApplicationContext context: Context,
        authStore: AuthStore,
        configStore: ConfigStore,
        okHttpClientFactory: OkHttpClientFactory,
        trackingDiskCache: TrackingDiskCache,
    ): ImageLoader {
        // Create retry interceptor with exponential backoff
        val retryInterceptor = ExponentialBackoffRetryInterceptor(
            maxRetries = 3,
            initialDelayMs = 500,
            maxDelayMs = 5000,
        )

        // Create OkHttpClient for our server with retry and auth
        val internalOkHttpClient = okHttpClientFactory
            .createBuilder(configStore.baseUrl.value)
            .addInterceptor(retryInterceptor)
            .addInterceptor(OkHttpAuthInterceptor(authStore))
            .build()

        // Create a plain OkHttpClient for external URLs (no auth, with retry)
        val externalOkHttpClient = OkHttpClient.Builder()
            .addInterceptor(retryInterceptor)
            .build()

        // Create a routing call factory that uses the appropriate client based on URL
        val routingCallFactory = RoutingCallFactory(
            configStore = configStore,
            internalClient = internalOkHttpClient,
            externalClient = externalOkHttpClient,
        )

        return ImageLoader.Builder(context)
            .components {
                add(CoilAuthTokenInterceptor(authStore, configStore))
                add(OkHttpNetworkFetcherFactory(callFactory = { routingCallFactory }))
            }
            .diskCache { trackingDiskCache }
            .build()
    }

    @Provides
    @Singleton
    fun provideImageCacheManager(
        trackingDiskCache: TrackingDiskCache,
    ): ImageCacheManager = CoilImageCacheManager(trackingDiskCache)

    @Provides
    @Singleton
    fun provideCacheManager(
        staticsCache: StaticsCache,
        staticsStore: StaticsStore,
        imageCacheManager: ImageCacheManager,
    ): CacheManager = CacheManagerImpl(staticsCache, staticsStore, imageCacheManager)

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
) : CoilInterceptor {

    override suspend fun intercept(chain: CoilInterceptor.Chain): ImageResult {
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
 * Internal URLs (to our server) use the internal client, external URLs use a plain client.
 */
private class RoutingCallFactory(
    private val configStore: ConfigStore,
    private val internalClient: OkHttpClient,
    private val externalClient: OkHttpClient,
) : okhttp3.Call.Factory {

    override fun newCall(request: okhttp3.Request): okhttp3.Call {
        val internalHost = extractHost(configStore.baseUrl.value)
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

/**
 * OkHttp interceptor that retries failed requests with exponential backoff.
 *
 * Retries on network errors (IOException) and server errors (5xx status codes).
 * Does NOT retry on client errors (4xx) as those indicate a problem with the request.
 *
 * @param maxRetries Maximum number of retry attempts
 * @param initialDelayMs Initial delay before first retry (milliseconds)
 * @param maxDelayMs Maximum delay between retries (milliseconds)
 */
private class ExponentialBackoffRetryInterceptor(
    private val maxRetries: Int = 3,
    private val initialDelayMs: Long = 500,
    private val maxDelayMs: Long = 5000,
) : OkHttpInterceptor {

    override fun intercept(chain: OkHttpInterceptor.Chain): Response {
        val request = chain.request()
        var lastException: IOException? = null
        var lastResponse: Response? = null

        repeat(maxRetries + 1) { attempt ->
            // Close previous response if any
            lastResponse?.close()

            try {
                val response = chain.proceed(request)

                // Success or client error - don't retry
                if (response.isSuccessful || response.code in 400..499) {
                    return response
                }

                // Server error (5xx) - retry
                if (response.code >= 500) {
                    lastResponse = response
                    if (attempt < maxRetries) {
                        val delay = calculateDelay(attempt)
                        Thread.sleep(delay)
                    }
                } else {
                    return response
                }
            } catch (e: IOException) {
                lastException = e
                if (attempt < maxRetries) {
                    val delay = calculateDelay(attempt)
                    Thread.sleep(delay)
                }
            }
        }

        // All retries exhausted
        lastResponse?.let { return it }
        throw lastException ?: IOException("Request failed after $maxRetries retries")
    }

    private fun calculateDelay(attempt: Int): Long {
        // Exponential backoff: initialDelay * 2^attempt
        val exponentialDelay = initialDelayMs * 2.0.pow(attempt.toDouble()).toLong()
        return min(exponentialDelay, maxDelayMs)
    }
}

/**
 * OkHttp interceptor that adds authentication token to requests.
 *
 * This interceptor adds the Authorization header to all requests using
 * the current auth token from AuthStore. If no token is available,
 * the request proceeds without auth (will likely fail with 401).
 */
private class OkHttpAuthInterceptor(
    private val authStore: AuthStore,
) : OkHttpInterceptor {

    override fun intercept(chain: OkHttpInterceptor.Chain): Response {
        val authToken = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken

        val requestWithAuth = if (authToken != null) {
            chain.request().newBuilder()
                .addHeader("Authorization", authToken)
                .build()
        } else {
            chain.request()
        }

        return chain.proceed(requestWithAuth)
    }
}
