package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.statics.StaticsSynchronizer
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

class InitializeApp @Inject internal constructor(
    private val initializers: Set<@JvmSuppressWildcards AppInitializer>,
    private val authStore: AuthStore,
    private val staticsSynchronizer: StaticsSynchronizer,
    private val syncManager: SyncManager,
    private val webSocketManager: WebSocketManager,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    operator fun invoke() {
        logger.info("invoke() initializing app with ${initializers.size} initializers")
        initializers.forEach { initializer ->
            val initializerName = initializer::class.simpleName
            logger.debug("invoke() running initializer: $initializerName")
            initializer.initialize()
            logger.debug("invoke() initializer completed: $initializerName")
        }
        logger.info("invoke() app initialization complete")

        // Wait for auth state to resolve, then initialize sync if logged in
        scope.launch {
            val authState = authStore.getAuthState().first { it !is AuthState.Loading }
            if (authState is AuthState.LoggedIn) {
                logger.info("invoke() user is logged in, initializing sync")
                webSocketManager.connect()
                syncManager.initialize()
            }
        }
    }
}