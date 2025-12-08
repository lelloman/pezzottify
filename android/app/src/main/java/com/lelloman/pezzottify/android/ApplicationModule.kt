package com.lelloman.pezzottify.android

import android.content.Context
import coil3.ImageLoader
import coil3.intercept.Interceptor
import coil3.network.httpHeaders
import coil3.request.ImageResult
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.logging.LogFileManager
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class ApplicationModule {

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
    ): ImageLoader {
        return ImageLoader.Builder(context)
            .components {
                add(CoilAuthTokenInterceptor(authStore))
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

private class CoilAuthTokenInterceptor(
    private val authStore: AuthStore,
) : Interceptor {

    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
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
}
