package com.lelloman.pezzottify.android.domain.websocket

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.lifecycle.AppLifecycleObserver
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class WebSocketInitializer @Inject constructor(
    private val authStore: AuthStore,
    private val webSocketManager: WebSocketManager,
    private val appLifecycleObserver: AppLifecycleObserver,
    private val networkConnectivityObserver: NetworkConnectivityObserver,
    private val player: PezzottifyPlayer,
) : AppInitializer {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var debounceJob: Job? = null

    override fun initialize() {
        scope.launch {
            combine(
                authStore.getAuthState(),
                appLifecycleObserver.isInForeground,
                networkConnectivityObserver.isNetworkAvailable,
                player.isPlaying,
            ) { authState, isInForeground, isNetworkAvailable, isPlaying ->
                ConnectionDecision(
                    isAuthenticated = authState is AuthState.LoggedIn,
                    isInForeground = isInForeground,
                    isNetworkAvailable = isNetworkAvailable,
                    isMusicPlaying = isPlaying,
                )
            }
                .distinctUntilChanged()
                .collect { decision ->
                    handleConnectionDecision(decision)
                }
        }
    }

    private fun handleConnectionDecision(decision: ConnectionDecision) {
        debounceJob?.cancel()
        debounceJob = scope.launch {
            // Debounce to avoid rapid connect/disconnect
            delay(DEBOUNCE_MS)

            val shouldConnect = decision.isAuthenticated &&
                decision.isNetworkAvailable &&
                (decision.isInForeground || decision.isMusicPlaying)

            val currentState = webSocketManager.connectionState.value
            val isConnected = currentState is ConnectionState.Connected ||
                currentState is ConnectionState.Connecting

            if (shouldConnect && !isConnected) {
                webSocketManager.connect()
            } else if (!shouldConnect && isConnected) {
                webSocketManager.disconnect()
            }
        }
    }

    private data class ConnectionDecision(
        val isAuthenticated: Boolean,
        val isInForeground: Boolean,
        val isNetworkAvailable: Boolean,
        val isMusicPlaying: Boolean,
    )

    companion object {
        private const val DEBOUNCE_MS = 500L
    }
}
