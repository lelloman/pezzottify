package com.lelloman.pezzottify.android.localdata

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.localdata.internal.AuthStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.ConfigStoreImpl
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticsDb
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticsItemFetchStateStoreImpl
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticsStoreImpl
import com.lelloman.pezzottify.android.logger.LoggerFactory
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
    fun provideConfigStore(
        @ApplicationContext context: Context,
        @DefaultHostUrl defaultHostUrl: String
    ): com.lelloman.pezzottify.android.domain.config.ConfigStore =
        ConfigStoreImpl(
            context = context,
            defaultHostUrl = defaultHostUrl,
        )

    @Provides
    @Singleton
    internal fun provideStaticsDb(@ApplicationContext context: Context): StaticsDb = Room
        .databaseBuilder(context, StaticsDb::class.java, StaticsDb.NAME)
        .build()

    @Provides
    @Singleton
    internal fun provideStaticsStore(
        staticsDb: StaticsDb,
        loggerFactory: LoggerFactory
    ): StaticsStore =
        StaticsStoreImpl(staticsDb, loggerFactory)

    @Provides
    @Singleton
    internal fun provideStaticItemFetchStateStore(
        staticsDb: StaticsDb
    ): StaticItemFetchStateStore =
        StaticsItemFetchStateStoreImpl(staticsDb.staticItemFetchStateDao())
}