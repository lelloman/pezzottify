package com.lelloman.pezzottify.android.localdata

import android.content.Context
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.sync.SyncStateStore
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.impression.ImpressionStore
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.notifications.NotificationLocalStore
import com.lelloman.pezzottify.android.domain.player.PlaybackStateStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.localdata.internal.auth.AuthStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.impression.ImpressionDao
import com.lelloman.pezzottify.android.localdata.internal.impression.ImpressionStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventDao
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.notifications.NotificationDao
import com.lelloman.pezzottify.android.localdata.internal.notifications.NotificationLocalStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.player.PlaybackStateStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.config.ConfigStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.settings.UserSettingsStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.skeleton.SkeletonStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.sync.SyncStateStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDb
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDbSizeCalculator
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsItemFetchStateStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.user.UserLocalDataDb
import com.lelloman.pezzottify.android.localdata.internal.user.UserDataStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.user.PermissionsStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.usercontent.UserContentDb
import com.lelloman.pezzottify.android.localdata.internal.usercontent.UserContentStoreImpl
import com.lelloman.pezzottify.android.localdata.internal.usercontent.UserPlaylistStoreImpl
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
class LocalDataModule {

    @Provides
    @Singleton
    fun provideAuthStore(
        @ApplicationContext context: Context,
        coroutineScope: CoroutineScope,
    ): AuthStore = AuthStoreImpl(context, coroutineScope)

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
        dbSizeCalculator: StaticsDbSizeCalculator,
        loggerFactory: LoggerFactory,
    ): StaticsStore = StaticsStoreImpl(staticsDb, dbSizeCalculator, loggerFactory)

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
        userLocalDataDb: UserLocalDataDb,
        timeProvider: TimeProvider,
    ): UserDataStore = UserDataStoreImpl(
        viewedContentDao = userLocalDataDb.viewedContentDao(),
        searchHistoryEntryDao = userLocalDataDb.searchHistoryEntryDao(),
        timeProvider = { timeProvider.nowUtcMs() },
    )

    @Provides
    @Singleton
    fun provideUserSettingsStore(
        @ApplicationContext context: Context
    ): UserSettingsStore = UserSettingsStoreImpl(context)

    @Provides
    @Singleton
    internal fun provideUserContentStore(
        userContentDb: UserContentDb
    ): UserContentStore = UserContentStoreImpl(userContentDb.likedContentDao())

    @Provides
    @Singleton
    internal fun provideUserPlaylistStore(
        userContentDb: UserContentDb
    ): UserPlaylistStore = UserPlaylistStoreImpl(userContentDb.playlistDao())

    @Provides
    @Singleton
    internal fun provideListeningEventDao(
        userContentDb: UserContentDb
    ): ListeningEventDao = userContentDb.listeningEventDao()

    @Provides
    @Singleton
    internal fun provideListeningEventStore(
        dao: ListeningEventDao
    ): ListeningEventStore = ListeningEventStoreImpl(dao)

    @Provides
    @Singleton
    fun provideSyncStateStore(
        @ApplicationContext context: Context
    ): SyncStateStore = SyncStateStoreImpl(context)

    @Provides
    @Singleton
    fun providePermissionsStore(
        @ApplicationContext context: Context
    ): PermissionsStore = PermissionsStoreImpl(context)

    @Provides
    @Singleton
    internal fun provideSkeletonStore(
        staticsDb: StaticsDb
    ): SkeletonStore = SkeletonStoreImpl(staticsDb.skeletonDao())

    @Provides
    @Singleton
    internal fun provideNotificationDao(
        userContentDb: UserContentDb
    ): NotificationDao = userContentDb.notificationDao()

    @Provides
    @Singleton
    internal fun provideNotificationLocalStore(
        notificationDao: NotificationDao
    ): NotificationLocalStore = NotificationLocalStoreImpl(notificationDao)

    @Provides
    @Singleton
    fun providePlaybackStateStore(
        @ApplicationContext context: Context
    ): PlaybackStateStore = PlaybackStateStoreImpl(context)

    @Provides
    @Singleton
    internal fun provideImpressionDao(
        userContentDb: UserContentDb
    ): ImpressionDao = userContentDb.impressionDao()

    @Provides
    @Singleton
    internal fun provideImpressionStore(
        dao: ImpressionDao
    ): ImpressionStore = ImpressionStoreImpl(dao)
}