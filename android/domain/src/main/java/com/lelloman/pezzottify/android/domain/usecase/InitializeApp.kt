package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.sync.StaticsSynchronizer
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

class InitializeApp @Inject internal constructor(
    private val initializers: Set<@JvmSuppressWildcards AppInitializer>,
    private val authStore: AuthStore,
    private val staticsSynchronizer: StaticsSynchronizer,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    operator fun invoke() {
        logger.info("invoke() initializing app with ${initializers.size} initializers")
        initializers.forEach { initializer ->
            val initializerName = initializer::class.simpleName
            logger.debug("invoke() running initializer: $initializerName")
            initializer.initialize()
            logger.debug("invoke() initializer completed: $initializerName")
        }
        logger.info("invoke() app initialization complete")
//        synchronizer.initialize()
//        runBlocking { authStore.initialize() }
    }
}