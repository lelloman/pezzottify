package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

class PerformLogout @Inject internal constructor(
    private val authStore: AuthStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val staticsCache: StaticsCache,
    private val userDataStore: UserDataStore,
    private val userContentStore: UserContentStore,
    private val permissionsStore: PermissionsStore,
    private val listeningEventStore: ListeningEventStore,
    private val syncManager: SyncManager,
    private val player: PezzottifyPlayer,
    private val webSocketManager: WebSocketManager,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    suspend operator fun invoke() {
        logger.info("invoke() starting logout")
        logger.debug("invoke() clearing player session")
        player.clearSession()
        logger.debug("invoke() disconnecting WebSocket")
        webSocketManager.disconnect()
        logger.debug("invoke() cleaning up sync manager")
        syncManager.cleanup()
        logger.debug("invoke() setting auth state to LoggedOut")
        authStore.storeAuthState(AuthState.LoggedOut)
        logger.debug("invoke() calling remote logout")
        remoteApiClient.logout()
        logger.debug("invoke() clearing statics cache")
        staticsCache.clearAll()
        logger.debug("invoke() deleting statics store")
        staticsStore.deleteAll()
        logger.debug("invoke() deleting user data store")
        userDataStore.deleteAll()
        logger.debug("invoke() deleting user content store")
        userContentStore.deleteAll()
        logger.debug("invoke() clearing permissions store")
        permissionsStore.clear()
        logger.debug("invoke() deleting listening event store")
        listeningEventStore.deleteAll()
        logger.info("invoke() logout complete")
    }
}