package com.lelloman.pezzottify.android.lifecycle

import android.app.Application
import com.lelloman.pezzottify.android.domain.lifecycle.AppLifecycleObserver
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class LifecycleModule {

    @Provides
    @Singleton
    fun provideAppLifecycleObserver(): AppLifecycleObserver =
        AndroidAppLifecycleObserver()

    @Provides
    @Singleton
    fun provideNetworkConnectivityObserver(
        application: Application
    ): NetworkConnectivityObserver =
        AndroidNetworkConnectivityObserver(application)
}
