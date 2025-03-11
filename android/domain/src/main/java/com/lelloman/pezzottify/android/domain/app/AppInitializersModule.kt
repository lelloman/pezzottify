package com.lelloman.pezzottify.android.domain.app

import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.sync.Synchronizer
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet

@Module
@InstallIn(SingletonComponent::class)
abstract class AppInitializersModule {

    @Binds
    @IntoSet
    internal abstract fun bindAuthStore(authStore: AuthStore): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsSynchronizer(synchronizer: Synchronizer): AppInitializer
}