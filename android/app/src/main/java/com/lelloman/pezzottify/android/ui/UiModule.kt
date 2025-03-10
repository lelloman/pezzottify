package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class UiModule {

    @Provides
    @Singleton
    fun provideContentResolver(staticsProvider: StaticsProvider): ContentResolver =
        UiContentResolver(staticsProvider)
}