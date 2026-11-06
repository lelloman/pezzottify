package com.lelloman.pezzottify.android

import com.lelloman.pezzottify.android.auth.OidcTokenRefresher
import com.lelloman.pezzottify.android.auth.SessionExpiredHandlerImpl
import com.lelloman.pezzottify.android.device.AndroidDeviceInfoProvider
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcConfig
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepositoryImpl
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepositoryImpl
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.sync.SyncManagerImpl
import com.lelloman.pezzottify.android.localdata.DefaultHostUrl
import com.lelloman.pezzottify.android.localdata.ServerConfigurable
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.oidc.AppAuthOidcManager
import com.lelloman.pezzottify.android.oidc.OkHttpDeviceFlowAuthManager
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class DomainModule {

    @Binds
    abstract fun bindDeviceInfoProvider(impl: AndroidDeviceInfoProvider): DeviceInfoProvider

    @Binds
    abstract fun bindSyncManager(impl: SyncManagerImpl): SyncManager

    @Binds
    @Singleton
    abstract fun bindDownloadStatusRepository(impl: DownloadStatusRepositoryImpl): DownloadStatusRepository

    @Binds
    @Singleton
    abstract fun bindNotificationRepository(impl: NotificationRepositoryImpl): NotificationRepository

    @Binds
    @Singleton
    abstract fun bindOidcAuthManager(impl: AppAuthOidcManager): OidcAuthManager

    @Binds
    @Singleton
    abstract fun bindSessionExpiredHandler(impl: SessionExpiredHandlerImpl): SessionExpiredHandler

    @Binds
    @Singleton
    abstract fun bindTokenRefresher(impl: OidcTokenRefresher): TokenRefresher

    companion object {
        @Provides
        @DefaultHostUrl
        fun provideDefaultHostUrl() = BuildConfig.DEFAULT_BASE_URL

        @Provides
        @ServerConfigurable
        fun provideServerConfigurable() = BuildConfig.SERVER_CONFIGURABLE

        @Provides
        @Singleton
        fun provideOidcConfig(): OidcConfig = OidcConfig(
            issuerUrl = BuildConfig.OIDC_ISSUER_URL,
            clientId = BuildConfig.OIDC_CLIENT_ID,
            redirectUri = "${BuildConfig.OIDC_REDIRECT_SCHEME}://oauth/callback",
        )

        @Provides
        @Singleton
        fun provideDeviceFlowAuthManager(
            appAuthOidcManager: AppAuthOidcManager,
            oidcConfig: OidcConfig,
            loggerFactory: LoggerFactory,
        ): DeviceFlowAuthManager = OkHttpDeviceFlowAuthManager(
            // Talks only to the OIDC issuer: no app auth headers, no retry interceptor
            // (the polling loop already retries at the server-mandated interval).
            httpClient = OkHttpClient.Builder()
                .connectTimeout(15, TimeUnit.SECONDS)
                .readTimeout(20, TimeUnit.SECONDS)
                .build(),
            endpointsProvider = appAuthOidcManager,
            oidcConfig = oidcConfig,
            loggerFactory = loggerFactory,
        )
    }
}
