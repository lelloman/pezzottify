package com.lelloman.pezzottify.android.domain.playback

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handler for playback WebSocket messages.
 *
 * Registers with WebSocketManager to handle "playback" type messages
 * and dispatches to RemotePlaybackManager for processing.
 */
@Singleton
class PlaybackWebSocketHandler @Inject constructor(
    private val webSocketManager: WebSocketManager,
    private val remotePlaybackManager: RemotePlaybackManagerImpl,
    loggerFactory: LoggerFactory,
    @IoDispatcher dispatcher: CoroutineDispatcher,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(PlaybackWebSocketHandler::class)
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private val json = Json {
        ignoreUnknownKeys = true
    }

    private val handler = MessageHandler { type, payload ->
        logger.debug("Received playback message: type=$type")
        if (payload != null) {
            handlePlaybackMessage(type, payload)
        } else {
            logger.warn("Playback message has no payload: $type")
        }
    }

    override fun initialize() {
        logger.info("Registering playback message handler")
        webSocketManager.registerHandler(PREFIX, handler)

        // Send hello when connected
        scope.launch {
            webSocketManager.connectionState.collectLatest { state ->
                if (state is ConnectionState.Connected) {
                    sendHello()
                }
            }
        }
    }

    private fun sendHello() {
        val deviceName = getDeviceName()
        val deviceType = "android"

        webSocketManager.send(
            "playback.hello",
            HelloPayload(
                deviceName = deviceName,
                deviceType = deviceType,
            ),
        )
        logger.info("Sent playback hello: $deviceName")
    }

    private fun getDeviceName(): String {
        val manufacturer = android.os.Build.MANUFACTURER.replaceFirstChar {
            if (it.isLowerCase()) it.titlecase() else it.toString()
        }
        val model = android.os.Build.MODEL
        return if (model.startsWith(manufacturer, ignoreCase = true)) {
            model
        } else {
            "$manufacturer $model"
        }
    }

    private fun handlePlaybackMessage(type: String, payloadString: String) {
        try {
            when (type) {
                "playback.welcome" -> {
                    val payload = json.decodeFromString<WelcomePayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleWelcome(payload) }
                }
                "playback.state" -> {
                    val state = json.decodeFromString<PlaybackState>(payloadString)
                    scope.launch { remotePlaybackManager.handleRemoteState(state) }
                }
                "playback.queue_sync", "playback.queue_update" -> {
                    val payload = json.decodeFromString<QueuePayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleQueueSync(payload) }
                }
                "playback.session_ended" -> {
                    val payload = json.decodeFromString<SessionEndedPayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleSessionEnded(payload.reason) }
                }
                "playback.device_list_changed" -> {
                    val payload = json.decodeFromString<DeviceListChangedPayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleDeviceListChanged(payload) }
                }
                "playback.command" -> {
                    val payload = json.decodeFromString<CommandPayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleCommand(payload) }
                }
                "playback.prepare_transfer" -> {
                    val payload = json.decodeFromString<PrepareTransferPayload>(payloadString)
                    scope.launch { remotePlaybackManager.handlePrepareTransfer(payload) }
                }
                "playback.become_audio_device" -> {
                    val payload = json.decodeFromString<BecomeAudioDevicePayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleBecomeAudioDevice(payload) }
                }
                "playback.transfer_complete" -> {
                    scope.launch { remotePlaybackManager.handleTransferComplete() }
                }
                "playback.transfer_aborted" -> {
                    val payload = json.decodeFromString<TransferAbortedPayload>(payloadString)
                    scope.launch { remotePlaybackManager.handleTransferAborted(payload) }
                }
                else -> {
                    logger.warn("Unknown playback message type: $type")
                }
            }
        } catch (e: Exception) {
            logger.error("Failed to parse playback message: $type, payload: $payloadString", e)
        }
    }

    companion object {
        private const val PREFIX = "playback"
    }
}
