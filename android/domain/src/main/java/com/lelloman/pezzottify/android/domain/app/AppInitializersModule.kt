package com.lelloman.pezzottify.android.domain.app

import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.impression.ImpressionSynchronizer
import com.lelloman.pezzottify.android.domain.listening.ListeningEventSynchronizer
import com.lelloman.pezzottify.android.domain.listening.ListeningTracker
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.settings.UserSettingsSynchronizer
import com.lelloman.pezzottify.android.domain.sync.StaticsSynchronizer
import com.lelloman.pezzottify.android.domain.sync.SyncWebSocketHandler
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.UserContentSynchronizer
import com.lelloman.pezzottify.android.domain.websocket.WebSocketInitializer
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet

@Module
@InstallIn(SingletonComponent::class)
abstract class AppInitializersModule {

    @Binds
    @IntoSet
    internal abstract fun bindAuthStore(authStore: AuthStore): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsStaticsSynchronizer(staticsSynchronizer: StaticsSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsPlayer(player: PezzottifyPlayer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsUserContentSynchronizer(synchronizer: UserContentSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsPlaylistSynchronizer(synchronizer: PlaylistSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsUserSettingsSynchronizer(synchronizer: UserSettingsSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsListeningTracker(tracker: ListeningTracker): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsListeningEventSynchronizer(synchronizer: ListeningEventSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsImpressionSynchronizer(synchronizer: ImpressionSynchronizer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsWebSocketInitializer(initializer: WebSocketInitializer): AppInitializer

    @Binds
    @IntoSet
    internal abstract fun bindsSyncWebSocketHandler(handler: SyncWebSocketHandler): AppInitializer
}