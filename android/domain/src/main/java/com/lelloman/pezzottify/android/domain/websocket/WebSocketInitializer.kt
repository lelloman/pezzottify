package com.lelloman.pezzottify.android.domain.websocket

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChangedBy
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class WebSocketInitializer @Inject constructor(
    private val authStore: AuthStore,
    private val webSocketManager: WebSocketManager,
) : AppInitializer {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun initialize() {
        scope.launch {
            authStore.getAuthState()
                .distinctUntilChangedBy { it::class }
                .collect { authState ->
                    when (authState) {
                        is AuthState.LoggedIn -> webSocketManager.connect()
                        is AuthState.LoggedOut -> webSocketManager.disconnect()
                        is AuthState.Loading -> { /* Do nothing while loading */ }
                    }
                }
        }
    }
}
