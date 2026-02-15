package com.lelloman.pezzottify.android.domain.websocket

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncManager
import com.lelloman.pezzottify.android.domain.lifecycle.AppLifecycleObserver
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.sync.SyncManager
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
class WebSocketInitializer internal constructor(
    private val authStore: AuthStore,
    private val webSocketManager: WebSocketManager,
    private val appLifecycleObserver: AppLifecycleObserver,
    private val networkConnectivityObserver: NetworkConnectivityObserver,
    private val player: PezzottifyPlayer,
    private val syncManager: SyncManager,
    private val catalogSyncManager: CatalogSyncManager,
    private val scope: CoroutineScope,
) : AppInitializer {

    @Inject
    constructor(
        authStore: AuthStore,
        webSocketManager: WebSocketManager,
        appLifecycleObserver: AppLifecycleObserver,
        networkConnectivityObserver: NetworkConnectivityObserver,
        player: PezzottifyPlayer,
        syncManager: SyncManager,
        catalogSyncManager: CatalogSyncManager,
    ) : this(
        authStore,
        webSocketManager,
        appLifecycleObserver,
        networkConnectivityObserver,
        player,
        syncManager,
        catalogSyncManager,
        CoroutineScope(SupervisorJob() + Dispatchers.IO),
    )

    private var debounceJob: Job? = null
    private var wasDisconnected = false

    override fun initialize() {
        // Observe connection/disconnection decisions
        scope.launch {
            combine(
                authStore.getAuthState(),
                appLifecycleObserver.isInForeground,
                networkConnectivityObserver.isNetworkAvailable,
                player.isPlaying,
                appLifecycleObserver.isKeptAliveExternally,
            ) { values ->
                ConnectionDecision(
                    isAuthenticated = values[0] is AuthState.LoggedIn,
                    isInForeground = values[1] as Boolean,
                    isNetworkAvailable = values[2] as Boolean,
                    isMusicPlaying = values[3] as Boolean,
                    isKeptAliveExternally = values[4] as Boolean,
                )
            }
                .distinctUntilChanged()
                .collect { decision ->
                    handleConnectionDecision(decision)
                }
        }

        // Observe WebSocket connection state to trigger sync catch-up on reconnection
        scope.launch {
            webSocketManager.connectionState.collect { state ->
                when (state) {
                    is ConnectionState.Connected -> {
                        if (wasDisconnected) {
                            // Reconnected after being disconnected - catch up on missed events
                            syncManager.catchUp()
                            catalogSyncManager.catchUp()
                        }
                        wasDisconnected = false
                    }
                    is ConnectionState.Disconnected,
                    is ConnectionState.Error -> {
                        wasDisconnected = true
                    }
                    is ConnectionState.Connecting -> {
                        // No state change needed while connecting
                    }
                }
            }
        }
    }

    private fun handleConnectionDecision(decision: ConnectionDecision) {
        debounceJob?.cancel()
        debounceJob = scope.launch {
            val shouldConnect = decision.isAuthenticated &&
                decision.isNetworkAvailable &&
                (decision.isInForeground || decision.isMusicPlaying || decision.isKeptAliveExternally)

            // Use a longer debounce for disconnect to handle the keep-alive → foreground
            // transition on Android TV: when PezzotTV (launcher) goes to background and unbinds
            // the KeepAliveService, isKeptAliveExternally becomes false before
            // ProcessLifecycleOwner updates isInForeground to true for the launching app.
            val debounceMs = if (shouldConnect) CONNECT_DEBOUNCE_MS else DISCONNECT_DEBOUNCE_MS
            delay(debounceMs)

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
        val isKeptAliveExternally: Boolean,
    )

    companion object {
        private const val CONNECT_DEBOUNCE_MS = 500L
        private const val DISCONNECT_DEBOUNCE_MS = 2000L
    }
}
