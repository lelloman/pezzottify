package com.lelloman.pezzottify.android

import com.lelloman.pezzottify.android.auth.SessionExpiredHandlerImpl
import com.lelloman.pezzottify.android.device.AndroidDeviceInfoProvider
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
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
import com.lelloman.pezzottify.android.oidc.AppAuthOidcManager
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
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

    companion object {
        @Provides
        @DefaultHostUrl
        fun provideDefaultHostUrl() = "http://10.0.2.2:3001"

        @Provides
        @Singleton
        fun provideOidcConfig(): OidcConfig = OidcConfig(
            issuerUrl = "https://auth.lelloman.com",
            clientId = "pezzottify-android",
            redirectUri = "com.lelloman.pezzottify.android://oauth/callback",
        )
    }
}