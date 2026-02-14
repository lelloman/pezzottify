package com.lelloman.pezzottify.android.domain.websocket

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncManager
import com.lelloman.pezzottify.android.domain.lifecycle.AppLifecycleObserver
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class WebSocketInitializerTest {

    private lateinit var authStore: AuthStore
    private lateinit var webSocketManager: WebSocketManager
    private lateinit var appLifecycleObserver: AppLifecycleObserver
    private lateinit var networkConnectivityObserver: NetworkConnectivityObserver
    private lateinit var player: PezzottifyPlayer
    private lateinit var syncManager: SyncManager
    private lateinit var catalogSyncManager: CatalogSyncManager

    private lateinit var authStateFlow: MutableStateFlow<AuthState>
    private lateinit var isInForegroundFlow: MutableStateFlow<Boolean>
    private lateinit var isNetworkAvailableFlow: MutableStateFlow<Boolean>
    private lateinit var isPlayingFlow: MutableStateFlow<Boolean>
    private lateinit var isKeptAliveExternallyFlow: MutableStateFlow<Boolean>
    private lateinit var connectionStateFlow: MutableStateFlow<ConnectionState>

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        webSocketManager = mockk(relaxed = true)
        appLifecycleObserver = mockk(relaxed = true)
        networkConnectivityObserver = mockk(relaxed = true)
        player = mockk(relaxed = true)
        syncManager = mockk(relaxed = true)
        catalogSyncManager = mockk(relaxed = true)

        authStateFlow = MutableStateFlow(AuthState.LoggedOut)
        isInForegroundFlow = MutableStateFlow(false)
        isNetworkAvailableFlow = MutableStateFlow(true)
        isPlayingFlow = MutableStateFlow(false)
        isKeptAliveExternallyFlow = MutableStateFlow(false)
        connectionStateFlow = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)

        every { authStore.getAuthState() } returns authStateFlow
        every { appLifecycleObserver.isInForeground } returns isInForegroundFlow
        every { appLifecycleObserver.isKeptAliveExternally } returns isKeptAliveExternallyFlow
        every { networkConnectivityObserver.isNetworkAvailable } returns isNetworkAvailableFlow
        every { player.isPlaying } returns isPlayingFlow
        every { webSocketManager.connectionState } returns connectionStateFlow

        coEvery { syncManager.catchUp() } returns true
    }

    private fun createWebSocketInitializer(scope: CoroutineScope) = WebSocketInitializer(
        authStore = authStore,
        webSocketManager = webSocketManager,
        appLifecycleObserver = appLifecycleObserver,
        networkConnectivityObserver = networkConnectivityObserver,
        player = player,
        syncManager = syncManager,
        catalogSyncManager = catalogSyncManager,
        scope = scope,
    )

    @Test
    fun `catchUp is called when WebSocket reconnects after being disconnected`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Simulate: was disconnected, now connected
            connectionStateFlow.value = ConnectionState.Disconnected
            advanceUntilIdle()

            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            coVerify { syncManager.catchUp() }
        } finally {
            initializerScope.cancel()
        }
    }

    @Test
    fun `catchUp is called when WebSocket connects after error state`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Simulate: error state, then connected
            connectionStateFlow.value = ConnectionState.Error("Connection failed")
            advanceUntilIdle()

            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            coVerify { syncManager.catchUp() }
        } finally {
            initializerScope.cancel()
        }
    }

    @Test
    fun `catchUp is called after disconnect and reconnect cycle`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Start connected
            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            // Disconnect (app goes to background)
            connectionStateFlow.value = ConnectionState.Disconnected
            advanceUntilIdle()

            // Reconnect (app comes to foreground)
            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            // Verify catchUp was called on reconnection
            coVerify(atLeast = 1) { syncManager.catchUp() }
        } finally {
            initializerScope.cancel()
        }
    }

    @Test
    fun `catchUp is not called when staying connected`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Connect
            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            // Clear any previous calls
            io.mockk.clearMocks(syncManager, answers = false)

            // Stay connected (same state emitted again - but StateFlow won't emit duplicate)
            // So we need to change to a different Connected instance
            connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0.0")
            advanceUntilIdle()

            // catchUp should not be called again since wasDisconnected is false
            coVerify(exactly = 0) { syncManager.catchUp() }
        } finally {
            initializerScope.cancel()
        }
    }

    @Test
    fun `connects when kept alive externally while not in foreground`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Authenticated, network available, not in foreground, not playing, but kept alive externally
            authStateFlow.value = AuthState.LoggedIn(
                userHandle = "user",
                authToken = "token",
                remoteUrl = "http://server",
            )
            isInForegroundFlow.value = false
            isPlayingFlow.value = false
            isKeptAliveExternallyFlow.value = true
            advanceUntilIdle()

            coVerify { webSocketManager.connect() }
        } finally {
            initializerScope.cancel()
        }
    }

    @Test
    fun `catchUp is not called when transitioning to Connecting state`() = runTest {
        val testDispatcher = StandardTestDispatcher(testScheduler)
        val initializerScope = CoroutineScope(Job() + testDispatcher)

        try {
            val webSocketInitializer = createWebSocketInitializer(initializerScope)

            webSocketInitializer.initialize()
            advanceUntilIdle()

            // Clear any previous calls
            io.mockk.clearMocks(syncManager, answers = false)

            // Transition to connecting
            connectionStateFlow.value = ConnectionState.Connecting
            advanceUntilIdle()

            // catchUp should not be called during connection attempt
            coVerify(exactly = 0) { syncManager.catchUp() }
        } finally {
            initializerScope.cancel()
        }
    }
}
