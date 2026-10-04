package com.lelloman.pezzottify.android.push

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.push.PushRegistration
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import javax.inject.Named
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class PushModule {

    @Binds
    abstract fun bindPushRegistration(impl: UnifiedPushRegistration): PushRegistration

    @Binds
    @IntoSet
    abstract fun bindPushInitializer(impl: UnifiedPushRegistration): AppInitializer

    @Binds
    abstract fun bindUnifiedPushClient(impl: AndroidUnifiedPushClient): UnifiedPushClient

    @Binds
    abstract fun bindPushPreferences(impl: SharedPreferencesPushPreferences): PushPreferences

    @Binds
    abstract fun bindPushWorkScheduler(impl: WorkManagerPushWorkScheduler): PushWorkScheduler

    companion object {
        @Provides
        @Singleton
        @Named(UnifiedPushRegistration.PUSH_SCOPE)
        fun providePushScope(): CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    }
}
