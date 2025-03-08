package com.lelloman.pezzottify.android.localdata

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.localdata.auth.AuthStore
import com.lelloman.pezzottify.android.localdata.auth.internal.AuthStoreImpl
import com.lelloman.pezzottify.android.localdata.config.ConfigStore
import com.lelloman.pezzottify.android.localdata.config.internal.ConfigStoreImpl
import com.lelloman.pezzottify.android.localdata.statics.StaticsStore
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticsDb
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticsStoreImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class LocalDataModule {

    @Provides
    @Singleton
    fun provideAuthStore(@ApplicationContext context: Context): AuthStore = AuthStoreImpl(context)

    @Provides
    @Singleton
    fun provideConfigStore(@ApplicationContext context: Context): ConfigStore =
        ConfigStoreImpl(context)

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .build()

    @Provides
    @Singleton
    internal fun provideStaticsStore(staticsDb: StaticsDb): StaticsStore = StaticsStoreImpl(
        staticsDao = staticsDb.staticsDao(),
        staticItemFetchStateDao = staticsDb.staticItemFetchStateDao(),
    )
}