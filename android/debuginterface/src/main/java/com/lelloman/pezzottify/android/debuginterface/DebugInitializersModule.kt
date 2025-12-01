package com.lelloman.pezzottify.android.debuginterface

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet

@Module
@InstallIn(SingletonComponent::class)
abstract class DebugInitializersModule {

    @Binds
    @IntoSet
    abstract fun bindDebugServerInitializer(initializer: DebugServerInitializer): AppInitializer
}
