package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.sync.StaticsSynchronizer
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

class InitializeApp @Inject internal constructor(
    private val initializers: Set<@JvmSuppressWildcards AppInitializer>,
    private val authStore: AuthStore,
    private val staticsSynchronizer: StaticsSynchronizer,
) : UseCase() {
    operator fun invoke() {
        initializers.forEach { it.initialize() }
//        synchronizer.initialize()
//        runBlocking { authStore.initialize() }
    }
}