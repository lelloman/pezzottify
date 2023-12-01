package com.lelloman.pezzottify.android.app.initializer

import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.login.LoginStateOperationsCollector
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet

@Module
@InstallIn(SingletonComponent::class)
class InitializersModule {

    @Provides
    @IntoSet
    fun providesCoilAppInitializer(loginManager: LoginManager): AppInitializer =
        CoilAppInitializer(loginManager)

    @Provides
    @IntoSet
    fun providesLoginStateOperationsCollectorAppInitializer(
        collector: LoginStateOperationsCollector,
        loginManager: LoginManager,
    ): AppInitializer = LoginStateOperationsCollectorAppInitializer(collector, loginManager)
}