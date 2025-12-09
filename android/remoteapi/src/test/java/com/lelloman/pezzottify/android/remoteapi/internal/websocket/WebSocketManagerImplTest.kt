package com.lelloman.pezzottify.android.remoteapi.internal.websocket

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.junit.Before
import org.junit.Test
import kotlin.reflect.KProperty

@OptIn(ExperimentalCoroutinesApi::class)
class WebSocketManagerImplTest {

    private lateinit var authStore: AuthStore
    private lateinit var configStore: ConfigStore
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var testScope: TestScope
    private lateinit var webSocketManager: WebSocketManagerImpl
    private lateinit var mockOkHttpClientFactory: OkHttpClientFactory
    private lateinit var mockOkHttpClient: OkHttpClient
    private lateinit var mockWebSocket: WebSocket

    private val authStateFlow = MutableStateFlow<AuthState>(AuthState.LoggedOut)
    private val baseUrlFlow = MutableStateFlow("http://localhost:3001")

    @Before
    fun setUp() {
        authStore = mockk()
        configStore = mockk()
        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        mockOkHttpClientFactory = mockk()
        mockOkHttpClient = mockk()
        mockWebSocket = mockk(relaxed = true)

        every { loggerFactory.getValue(any(), any<KProperty<*>>()) } returns mockLogger
        every { authStore.getAuthState() } returns authStateFlow
        every { configStore.baseUrl } returns baseUrlFlow
        every { mockOkHttpClientFactory.createBuilder(any()) } returns OkHttpClient.Builder()
        every { mockOkHttpClient.newWebSocket(any<Request>(), any<WebSocketListener>()) } returns mockWebSocket

        testScope = TestScope(StandardTestDispatcher())

        webSocketManager = WebSocketManagerImpl(
            authStore = authStore,
            configStore = configStore,
            okHttpClientFactory = mockOkHttpClientFactory,
            coroutineScope = testScope,
            loggerFactory = loggerFactory,
        )
    }

    private fun loggedInState() = AuthState.LoggedIn(
        userHandle = "test-user",
        authToken = "test-token",
        remoteUrl = "http://localhost:3001"
    )

    // =========================================================================
    // Connection State Tests
    // =========================================================================

    @Test
    fun `initial state is Disconnected`() {
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Disconnected)
    }

    @Test
    fun `connect sets error state when not logged in`() = testScope.runTest {
        authStateFlow.value = AuthState.LoggedOut

        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isInstanceOf(ConnectionState.Error::class.java)
        assertThat((webSocketManager.connectionState.value as ConnectionState.Error).message)
            .isEqualTo("Not logged in")
    }

    @Test
    fun `connect sets state to Connecting when logged in`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    @Test
    fun `connect is idempotent when already connecting`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)

        // Second connect should be a no-op
        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    @Test
    fun `disconnect sets state to Disconnected`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.disconnect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Disconnected)
    }

    @Test
    fun `disconnect allows reconnection`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        // First connect
        webSocketManager.connect()
        advanceUntilIdle()
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)

        // Disconnect
        webSocketManager.disconnect()
        advanceUntilIdle()
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Disconnected)

        // Reconnect should work
        webSocketManager.connect()
        advanceUntilIdle()
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    // =========================================================================
    // Handler Registration Tests
    // =========================================================================

    @Test
    fun `registerHandler does not throw`() {
        val handler = mockk<MessageHandler>(relaxed = true)
        webSocketManager.registerHandler("test", handler)
        // No exception means success
    }

    @Test
    fun `unregisterHandler does not throw`() {
        val handler = mockk<MessageHandler>(relaxed = true)
        webSocketManager.registerHandler("test", handler)
        webSocketManager.unregisterHandler("test")
        // No exception means success
    }

    @Test
    fun `unregisterHandler for non-existent handler does not throw`() {
        webSocketManager.unregisterHandler("non-existent")
        // No exception means success
    }

    // =========================================================================
    // URL Building Tests
    // =========================================================================

    @Test
    fun `connect works with http base URL`() = testScope.runTest {
        baseUrlFlow.value = "http://localhost:3001"
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        // The URL building is internal, but we can verify connect was attempted
        // by checking the state transitioned to Connecting
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    @Test
    fun `connect works with https base URL`() = testScope.runTest {
        baseUrlFlow.value = "https://example.com"
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    @Test
    fun `connect works with trailing slash in base URL`() = testScope.runTest {
        baseUrlFlow.value = "http://localhost:3001/"
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Connecting)
    }

    // =========================================================================
    // Send Message Tests
    // =========================================================================

    @Test
    fun `send does not throw when not connected`() = testScope.runTest {
        // Not connected, send should be a no-op
        webSocketManager.send("ping", null)
        // No exception means success
    }

    @Test
    fun `send with null payload does not throw`() = testScope.runTest {
        authStateFlow.value = loggedInState()
        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.send("test", null)
        // No exception means success
    }

    @Test
    fun `send with string payload does not throw`() = testScope.runTest {
        authStateFlow.value = loggedInState()
        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.send("test", "payload")
        // No exception means success
    }

    @Test
    fun `send with int payload does not throw`() = testScope.runTest {
        authStateFlow.value = loggedInState()
        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.send("test", 123)
        // No exception means success
    }

    @Test
    fun `send with boolean payload does not throw`() = testScope.runTest {
        authStateFlow.value = loggedInState()
        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.send("test", true)
        // No exception means success
    }

    @Test
    fun `send with map payload does not throw`() = testScope.runTest {
        authStateFlow.value = loggedInState()
        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.send("test", mapOf("key" to "value"))
        // No exception means success
    }

    // =========================================================================
    // Reconnection Backoff Tests
    // =========================================================================

    @Test
    fun `calculateBackoff returns minBackoff on first attempt`() {
        // Use reflection to test private method
        val method = WebSocketManagerImpl::class.java.getDeclaredMethod("calculateBackoff")
        method.isAccessible = true

        val backoff = method.invoke(webSocketManager) as Long

        assertThat(backoff).isEqualTo(1000L) // MIN_BACKOFF_MS
    }

    @Test
    fun `calculateBackoff increases exponentially`() {
        val reconnectAttemptField = WebSocketManagerImpl::class.java.getDeclaredField("reconnectAttempt")
        reconnectAttemptField.isAccessible = true

        val method = WebSocketManagerImpl::class.java.getDeclaredMethod("calculateBackoff")
        method.isAccessible = true

        // First attempt: 1000 * 1.5^0 = 1000
        reconnectAttemptField.setInt(webSocketManager, 0)
        val backoff1 = method.invoke(webSocketManager) as Long
        assertThat(backoff1).isEqualTo(1000L)

        // Second attempt: 1000 * 1.5^1 = 1500
        reconnectAttemptField.setInt(webSocketManager, 1)
        val backoff2 = method.invoke(webSocketManager) as Long
        assertThat(backoff2).isEqualTo(1500L)

        // Third attempt: 1000 * 1.5^2 = 2250
        reconnectAttemptField.setInt(webSocketManager, 2)
        val backoff3 = method.invoke(webSocketManager) as Long
        assertThat(backoff3).isEqualTo(2250L)
    }

    @Test
    fun `calculateBackoff caps at maxBackoff`() {
        val reconnectAttemptField = WebSocketManagerImpl::class.java.getDeclaredField("reconnectAttempt")
        reconnectAttemptField.isAccessible = true

        val method = WebSocketManagerImpl::class.java.getDeclaredMethod("calculateBackoff")
        method.isAccessible = true

        // High attempt count should cap at MAX_BACKOFF_MS (30000)
        reconnectAttemptField.setInt(webSocketManager, 100)
        val backoff = method.invoke(webSocketManager) as Long
        assertThat(backoff).isEqualTo(30000L)
    }

    // =========================================================================
    // Intentional Disconnect Tests
    // =========================================================================

    @Test
    fun `disconnect sets intentionalDisconnect flag`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        webSocketManager.connect()
        advanceUntilIdle()

        webSocketManager.disconnect()
        advanceUntilIdle()

        // Verify state is Disconnected (not Error)
        assertThat(webSocketManager.connectionState.value).isEqualTo(ConnectionState.Disconnected)

        // Verify intentionalDisconnect flag via reflection
        val field = WebSocketManagerImpl::class.java.getDeclaredField("intentionalDisconnect")
        field.isAccessible = true
        assertThat(field.getBoolean(webSocketManager)).isTrue()
    }

    @Test
    fun `connect resets intentionalDisconnect flag`() = testScope.runTest {
        authStateFlow.value = loggedInState()

        // First connect and disconnect
        webSocketManager.connect()
        advanceUntilIdle()
        webSocketManager.disconnect()
        advanceUntilIdle()

        // Reconnect
        webSocketManager.connect()
        advanceUntilIdle()

        // Verify intentionalDisconnect flag is reset
        val field = WebSocketManagerImpl::class.java.getDeclaredField("intentionalDisconnect")
        field.isAccessible = true
        assertThat(field.getBoolean(webSocketManager)).isFalse()
    }
}
