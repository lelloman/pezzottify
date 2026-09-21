package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import com.lelloman.pezzottify.android.domain.equalizer.EqualizerStore
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object EqualizerModule {
    @Provides
    @Singleton
    fun provideEqualizerStore(@ApplicationContext context: Context): EqualizerStore = EqualizerStoreImpl(context)
}
