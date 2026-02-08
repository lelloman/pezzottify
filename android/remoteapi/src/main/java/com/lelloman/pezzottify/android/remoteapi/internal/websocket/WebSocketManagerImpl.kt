package com.lelloman.pezzottify.android.remoteapi.internal.websocket

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.websocket.ClientMessage
import com.lelloman.pezzottify.android.domain.websocket.ConnectedPayload
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.ErrorPayload
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.ServerMessage
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.encodeToJsonElement
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit

internal class WebSocketManagerImpl(
    private val authStore: AuthStore,
    private val configStore: ConfigStore,
    private val okHttpClientFactory: OkHttpClientFactory,
    private val tokenRefresher: TokenRefresher,
    private val sessionExpiredHandler: SessionExpiredHandler,
    private val coroutineScope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : WebSocketManager {

    private val logger: Logger by loggerFactory

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    private var okHttpClient: OkHttpClient? = null

    private val _connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    override val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()

    private val handlers = ConcurrentHashMap<String, MessageHandler>()

    private var webSocket: WebSocket? = null
    private var reconnectJob: Job? = null
    private var heartbeatJob: Job? = null
    private var reconnectAttempt = 0
    private var intentionalDisconnect = false
    private var hasAttemptedTokenRefresh = false

    override suspend fun connect() {
        if (_connectionState.value is ConnectionState.Connecting ||
            _connectionState.value is ConnectionState.Connected
        ) {
            logger.debug("Already connected or connecting, skipping connect()")
            return
        }

        val authState = authStore.getAuthState().value
        if (authState !is AuthState.LoggedIn) {
            logger.warn("Cannot connect WebSocket: not logged in")
            _connectionState.value = ConnectionState.Error("Not logged in")
            return
        }

        // Reset token refresh flag on new connection attempt
        hasAttemptedTokenRefresh = false

        intentionalDisconnect = false
        _connectionState.value = ConnectionState.Connecting

        // Ensure we have a fresh token before connecting
        val authToken = ensureFreshToken(authState)
        if (authToken == null) {
            logger.warn("Cannot connect WebSocket: failed to get valid token")
            _connectionState.value = ConnectionState.Error("Authentication failed")
            sessionExpiredHandler.onSessionExpired()
            return
        }

        val baseUrl = configStore.baseUrl.value
        val wsUrl = buildWebSocketUrl(baseUrl)

        logger.info("Connecting to WebSocket: $wsUrl")

        val client = okHttpClientFactory.createBuilder(baseUrl)
            .pingInterval(PING_INTERVAL_SECONDS, TimeUnit.SECONDS)
            .build()
        okHttpClient = client

        val request = Request.Builder()
            .url(wsUrl)
            .header("Authorization", authToken)
            .build()

        webSocket = client.newWebSocket(request, createWebSocketListener())
    }

    /**
     * Ensures we have a fresh auth token before connecting.
     * If a refresh token is available, proactively refreshes to avoid connection failures.
     */
    private suspend fun ensureFreshToken(authState: AuthState.LoggedIn): String? {
        // If we have a refresh token, proactively refresh to ensure fresh tokens
        if (authState.refreshToken != null) {
            logger.debug("Proactively refreshing token before WebSocket connect")
            when (val result = tokenRefresher.refreshTokens()) {
                is TokenRefresher.RefreshResult.Success -> {
                    logger.info("Token refresh successful, using new token for WebSocket")
                    return result.newAuthToken
                }
                is TokenRefresher.RefreshResult.Failed -> {
                    logger.warn("Token refresh failed: ${result.reason}, trying with existing token")
                    // Fall through to use existing token
                }
                is TokenRefresher.RefreshResult.NotAvailable -> {
                    logger.debug("No refresh available, using existing token")
                    // Fall through to use existing token
                }
                is TokenRefresher.RefreshResult.RateLimited -> {
                    logger.warn("Token refresh rate limited, trying with existing token")
                    // Fall through to use existing token - will retry on next reconnect
                }
            }
        }

        // Use existing token (may work if not expired)
        return authState.authToken
    }

    override suspend fun disconnect() {
        logger.info("Disconnecting WebSocket intentionally")
        intentionalDisconnect = true
        reconnectJob?.cancel()
        reconnectJob = null
        heartbeatJob?.cancel()
        heartbeatJob = null
        webSocket?.close(CLOSE_NORMAL, "Client disconnected")
        webSocket = null
        _connectionState.value = ConnectionState.Disconnected
    }

    override fun send(type: String, payload: Any?) {
        val ws = webSocket
        if (ws == null || _connectionState.value !is ConnectionState.Connected) {
            logger.warn("Cannot send message: not connected")
            return
        }

        val jsonPayload = anyToJsonElement(payload)

        val message = ClientMessage(type = type, payload = jsonPayload)
        val messageJson = json.encodeToString(message)
        logger.debug("Sending message: $messageJson")
        ws.send(messageJson)
    }

    private fun anyToJsonElement(value: Any?): JsonElement = when (value) {
        null -> JsonNull
        is JsonElement -> value
        is String -> JsonPrimitive(value)
        is Int -> JsonPrimitive(value)
        is Long -> JsonPrimitive(value)
        is Double -> JsonPrimitive(value)
        is Float -> JsonPrimitive(value)
        is Boolean -> JsonPrimitive(value)
        is Map<*, *> -> JsonObject(value.entries.associate { (k, v) ->
            (k as String) to anyToJsonElement(v)
        })
        is List<*> -> JsonArray(value.map { anyToJsonElement(it) })
        else -> {
            logger.warn("Unsupported payload type: ${value::class.simpleName}")
            JsonNull
        }
    }

    override fun registerHandler(prefix: String, handler: MessageHandler) {
        handlers[prefix] = handler
        logger.debug("Registered handler for prefix: $prefix")
    }

    override fun unregisterHandler(prefix: String) {
        handlers.remove(prefix)
        logger.debug("Unregistered handler for prefix: $prefix")
    }

    private fun createWebSocketListener() = object : WebSocketListener() {

        override fun onOpen(webSocket: WebSocket, response: Response) {
            logger.info("WebSocket connection opened")
            // State will be set to Connected when we receive the "connected" message
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            logger.debug("Received message: $text")
            handleMessage(text)
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            logger.info("WebSocket closing: code=$code, reason=$reason")
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            logger.info("WebSocket closed: code=$code, reason=$reason")
            heartbeatJob?.cancel()
            heartbeatJob = null
            this@WebSocketManagerImpl.webSocket = null

            if (!intentionalDisconnect) {
                // Unexpected close - treat as error and attempt reconnection
                _connectionState.value = ConnectionState.Error(reason.ifEmpty { "Connection closed unexpectedly" })
                scheduleReconnect()
            }
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            heartbeatJob?.cancel()
            heartbeatJob = null
            this@WebSocketManagerImpl.webSocket = null

            if (intentionalDisconnect) {
                // Failure during intentional disconnect (e.g., EOFException when closing)
                // This is expected, just log at debug level and set Disconnected state
                logger.debug("WebSocket closed during intentional disconnect: ${t.message}")
                _connectionState.value = ConnectionState.Disconnected
            } else {
                // Check if this is an auth failure (401/403)
                val responseCode = response?.code
                if (responseCode == 401 || responseCode == 403) {
                    logger.warn("WebSocket auth failure (HTTP $responseCode), attempting token refresh")
                    handleAuthFailure()
                } else {
                    // Unexpected failure - log error, set Error state, and attempt reconnection
                    logger.error("WebSocket failure: ${t.message}", t)
                    _connectionState.value = ConnectionState.Error(t.message ?: "Connection failed")
                    scheduleReconnect()
                }
            }
        }
    }

    /**
     * Handles WebSocket authentication failures by attempting token refresh and retry.
     */
    private fun handleAuthFailure() {
        if (hasAttemptedTokenRefresh) {
            // Already tried refreshing, give up and trigger logout
            logger.warn("Token refresh already attempted, triggering session expired")
            _connectionState.value = ConnectionState.Error("Authentication failed")
            sessionExpiredHandler.onSessionExpired()
            return
        }

        hasAttemptedTokenRefresh = true
        coroutineScope.launch {
            logger.info("Attempting token refresh after WebSocket auth failure")
            when (val result = tokenRefresher.refreshTokens()) {
                is TokenRefresher.RefreshResult.Success -> {
                    logger.info("Token refresh successful, retrying WebSocket connection")
                    // Reset state and reconnect
                    _connectionState.value = ConnectionState.Disconnected
                    connect()
                }
                is TokenRefresher.RefreshResult.Failed -> {
                    logger.warn("Token refresh failed: ${result.reason}, triggering session expired")
                    _connectionState.value = ConnectionState.Error("Authentication failed")
                    sessionExpiredHandler.onSessionExpired()
                }
                is TokenRefresher.RefreshResult.NotAvailable -> {
                    logger.warn("No refresh token available, triggering session expired")
                    _connectionState.value = ConnectionState.Error("Authentication failed")
                    sessionExpiredHandler.onSessionExpired()
                }
                is TokenRefresher.RefreshResult.RateLimited -> {
                    logger.warn("Token refresh rate limited for ${result.retryAfterMs}ms, will retry on next reconnect")
                    _connectionState.value = ConnectionState.Error("Rate limited - please wait")
                    // Don't trigger session expired - this is temporary, reconnect will retry
                }
            }
        }
    }

    private fun handleMessage(text: String) {
        val serverMessage = try {
            json.decodeFromString<ServerMessage>(text)
        } catch (e: Exception) {
            logger.error("Failed to parse server message: $text", e)
            return
        }

        when (serverMessage.type) {
            "connected" -> handleConnectedMessage(serverMessage)
            "pong" -> logger.debug("Received pong")
            "error" -> handleErrorMessage(serverMessage)
            else -> dispatchToHandlers(serverMessage)
        }
    }

    private fun handleConnectedMessage(message: ServerMessage) {
        val payload = message.payload?.let {
            try {
                json.decodeFromJsonElement(ConnectedPayload.serializer(), it)
            } catch (e: Exception) {
                logger.error("Failed to parse connected payload", e)
                null
            }
        }

        val deviceId = payload?.device_id ?: 0
        val serverVersion = payload?.server_version ?: "unknown"
        logger.info("WebSocket connected with device_id: $deviceId, server_version: $serverVersion")

        reconnectAttempt = 0
        _connectionState.value = ConnectionState.Connected(deviceId, serverVersion)
        startHeartbeat()
    }

    private fun handleErrorMessage(message: ServerMessage) {
        val payload = message.payload?.let {
            try {
                json.decodeFromJsonElement(ErrorPayload.serializer(), it)
            } catch (e: Exception) {
                logger.error("Failed to parse error payload", e)
                null
            }
        }

        logger.error("Server error: code=${payload?.code}, message=${payload?.message}")
    }

    private fun dispatchToHandlers(message: ServerMessage) {
        val type = message.type
        val prefix = type.substringBefore(".")

        val handler = handlers[prefix]
        if (handler != null) {
            val payloadString = message.payload?.toString()
            handler.onMessage(type, payloadString)
        } else {
            logger.debug("No handler registered for prefix: $prefix (type: $type)")
        }
    }

    private fun startHeartbeat() {
        heartbeatJob?.cancel()
        heartbeatJob = coroutineScope.launch {
            while (true) {
                delay(HEARTBEAT_INTERVAL_MS)
                if (_connectionState.value is ConnectionState.Connected) {
                    send("ping", null)
                } else {
                    break
                }
            }
        }
    }

    private fun scheduleReconnect() {
        reconnectJob?.cancel()
        reconnectJob = coroutineScope.launch {
            val backoffMs = calculateBackoff()
            logger.info("Scheduling reconnect in ${backoffMs}ms (attempt ${reconnectAttempt + 1})")
            delay(backoffMs)
            reconnectAttempt++
            connect()
        }
    }

    private fun calculateBackoff(): Long {
        val backoff = (MIN_BACKOFF_MS * Math.pow(BACKOFF_MULTIPLIER, reconnectAttempt.toDouble())).toLong()
        return minOf(backoff, MAX_BACKOFF_MS)
    }

    private fun buildWebSocketUrl(baseUrl: String): String {
        val wsUrl = baseUrl
            .replace("https://", "wss://")
            .replace("http://", "ws://")
            .trimEnd('/')
        return "$wsUrl/v1/ws"
    }

    companion object {
        private const val CLOSE_NORMAL = 1000
        private const val MIN_BACKOFF_MS = 1000L
        private const val MAX_BACKOFF_MS = 30000L
        private const val BACKOFF_MULTIPLIER = 1.5
        private const val HEARTBEAT_INTERVAL_MS = 30000L
        private const val PING_INTERVAL_SECONDS = 30L
    }
}
