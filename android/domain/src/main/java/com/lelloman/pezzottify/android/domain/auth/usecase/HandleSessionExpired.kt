package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredEventBus
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject

/**
 * Use case that handles session expiration.
 * Similar to PerformLogout but:
 * - Doesn't call the remote logout endpoint (session is already invalid)
 * - Exposes the event flow for UI to observe
 */
class HandleSessionExpired @Inject internal constructor(
    private val authStore: AuthStore,
    private val staticsStore: StaticsStore,
    private val staticsCache: StaticsCache,
    private val skeletonStore: SkeletonStore,
    private val userDataStore: UserDataStore,
    private val userContentStore: UserContentStore,
    private val permissionsStore: PermissionsStore,
    private val listeningEventStore: ListeningEventStore,
    private val syncManager: SyncManager,
    private val player: PezzottifyPlayer,
    private val webSocketManager: WebSocketManager,
    private val sessionExpiredEventBus: SessionExpiredEventBus,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Flow of session expired events. UI should collect this and
     * navigate to login when an event is emitted.
     */
    val events: Flow<Unit> = sessionExpiredEventBus.events

    /**
     * Perform cleanup when session has expired.
     * Does NOT call remote logout since session is already invalid.
     */
    suspend operator fun invoke() {
        logger.info("invoke() starting session expired cleanup")

        // Don't cleanup if already logged out
        if (authStore.getAuthState().value is AuthState.LoggedOut) {
            logger.info("invoke() already logged out, skipping")
            return
        }

        logger.debug("invoke() clearing player session")
        player.clearSession()
        logger.debug("invoke() disconnecting WebSocket")
        webSocketManager.disconnect()
        logger.debug("invoke() cleaning up sync manager")
        syncManager.cleanup()
        logger.debug("invoke() setting auth state to LoggedOut")
        authStore.storeAuthState(AuthState.LoggedOut)
        // Skip remote logout - session is already invalid on server
        logger.debug("invoke() clearing statics cache")
        staticsCache.clearAll()
        logger.debug("invoke() deleting statics store")
        staticsStore.deleteAll()
        logger.debug("invoke() clearing skeleton store")
        skeletonStore.clear()
        logger.debug("invoke() deleting user data store")
        userDataStore.deleteAll()
        logger.debug("invoke() deleting user content store")
        userContentStore.deleteAll()
        logger.debug("invoke() clearing permissions store")
        permissionsStore.clear()
        logger.debug("invoke() deleting listening event store")
        listeningEventStore.deleteAll()
        logger.info("invoke() session expired cleanup complete")
    }
}
