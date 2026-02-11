package com.lelloman.pezzottify.android.sync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet

@Module
@InstallIn(SingletonComponent::class)
abstract class BackgroundSyncModule {

    @Binds
    @IntoSet
    abstract fun bindBackgroundSyncScheduler(scheduler: BackgroundSyncScheduler): AppInitializer
}
