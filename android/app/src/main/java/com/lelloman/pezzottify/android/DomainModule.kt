package com.lelloman.pezzottify.android

import com.lelloman.pezzottify.android.device.AndroidDeviceInfoProvider
import com.lelloman.pezzottify.android.domain.config.SslPinConfig
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.sync.SyncManagerImpl
import com.lelloman.pezzottify.android.localdata.DefaultHostUrl
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

    companion object {
        @Provides
        @DefaultHostUrl
        fun provideDefaultHostUrl() = "http://10.0.2.2:3001"

        @Provides
        @Singleton
        fun provideSslPinConfig(): SslPinConfig = object : SslPinConfig {
            override val pinHash: String = BuildConfig.SSL_PIN_HASH
        }
    }
}