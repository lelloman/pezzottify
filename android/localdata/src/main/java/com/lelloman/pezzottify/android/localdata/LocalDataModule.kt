package com.lelloman.pezzottify.android.localdata

import android.content.Context
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.localdata.internal.auth.AuthStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.config.ConfigStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDb
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsItemFetchStateStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.user.UserDataDb
import com.lelloman.pezzottify.android.localdata.internal.user.UserDataStoreImpl
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
    ): ConfigStore = ConfigStoreImpl(
        context = context,
        defaultHostUrl = defaultHostUrl,
    )

    @Provides
    @Singleton
    internal fun provideStaticsStore(
        staticsDb: StaticsDb,
        loggerFactory: LoggerFactory
    ): StaticsStore = StaticsStoreImpl(staticsDb, loggerFactory)

    @Provides
    @Singleton
    internal fun provideStaticItemFetchStateStore(
        staticsDb: StaticsDb,
        timeProvider: TimeProvider,
    ): StaticItemFetchStateStore =
        StaticsItemFetchStateStoreImpl(staticsDb.staticItemFetchStateDao(), timeProvider)

    @Provides
    @Singleton
    internal fun provideUserDataStore(
        userDataDb: UserDataDb
    ): UserDataStore = UserDataStoreImpl(userDataDb.viewedContentDao())
}