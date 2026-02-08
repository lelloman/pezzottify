package com.lelloman.pezzottify.android.domain.playbacksession

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.util.concurrent.atomic.AtomicInteger
import javax.inject.Inject
import javax.inject.Singleton

@OptIn(DelicateCoroutinesApi::class)
@Singleton
class PlaybackSessionHandler internal constructor(
    private val webSocketManager: WebSocketManager,
    private val player: PezzottifyPlayer,
    private val playbackMetadataProvider: PlaybackMetadataProvider,
    private val deviceInfoProvider: DeviceInfoProvider,
    private val timeProvider: TimeProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    @Inject
    constructor(
        webSocketManager: WebSocketManager,
        player: PezzottifyPlayer,
        playbackMetadataProvider: PlaybackMetadataProvider,
        deviceInfoProvider: DeviceInfoProvider,
        timeProvider: TimeProvider,
        loggerFactory: LoggerFactory,
    ) : this(
        webSocketManager,
        player,
        playbackMetadataProvider,
        deviceInfoProvider,
        timeProvider,
        GlobalScope,
        loggerFactory,
    )

    private val logger = loggerFactory.getLogger(PlaybackSessionHandler::class)
    private val json = Json { ignoreUnknownKeys = true }

    private var isBroadcasting = false
    private val queueVersion = AtomicInteger(0)
    private var broadcastJob: Job? = null

    private val handler = MessageHandler { type, payload ->
        handleMessage(type, payload)
    }

    override fun initialize() {
        webSocketManager.registerHandler(PREFIX, handler)

        scope.launch {
            webSocketManager.connectionState.collect { state ->
                when (state) {
                    is ConnectionState.Connected -> sendHello()
                    is ConnectionState.Disconnected,
                    is ConnectionState.Error -> stopBroadcasting()
                    is ConnectionState.Connecting -> {}
                }
            }
        }

        scope.launch {
            combine(
                player.isActive,
                player.isPlaying,
                player.currentTrackIndex,
                player.volumeState,
                player.shuffleEnabled,
                player.repeatMode,
            ) { values ->
                PlayerStateSnapshot(
                    isActive = values[0] as Boolean,
                    isPlaying = values[1] as Boolean,
                    trackIndex = values[2] as Int?,
                    volumeState = values[3] as VolumeState,
                    shuffleEnabled = values[4] as Boolean,
                    repeatMode = values[5] as RepeatMode,
                )
            }.collect { snapshot ->
                if (snapshot.isActive) {
                    if (!isBroadcasting) {
                        startBroadcasting()
                    }
                    broadcastState()
                } else {
                    if (isBroadcasting) {
                        sendStoppedState()
                        stopBroadcasting()
                    }
                }
            }
        }

        scope.launch {
            player.playbackPlaylist.collect { playlist ->
                if (playlist != null && isBroadcasting) {
                    broadcastQueue(playlist.tracksIds)
                }
            }
        }
    }

    private fun sendHello() {
        val deviceInfo = deviceInfoProvider.getDeviceInfo()
        webSocketManager.send(
            "$PREFIX.hello",
            mapOf(
                "device_name" to (deviceInfo.deviceName ?: "Android"),
                "device_type" to "android",
            )
        )
        logger.debug("Sent playback hello")
        if (player.isActive.value) {
            startBroadcasting()
            broadcastState()
        }
    }

    private fun startBroadcasting() {
        if (isBroadcasting) return
        isBroadcasting = true
        broadcastJob = scope.launch {
            while (true) {
                delay(BROADCAST_INTERVAL_MS)
                if (isBroadcasting) {
                    broadcastState()
                }
            }
        }
        logger.debug("Started periodic broadcasting")
    }

    private fun stopBroadcasting() {
        if (!isBroadcasting) return
        isBroadcasting = false
        broadcastJob?.cancel()
        broadcastJob = null
        logger.debug("Stopped periodic broadcasting")
    }

    private fun broadcastState() {
        if (webSocketManager.connectionState.value !is ConnectionState.Connected) return

        val queueState = playbackMetadataProvider.queueState.value ?: return
        val currentTrack = queueState.currentTrack ?: return

        val currentTrackMap = mapOf<String, Any?>(
            "id" to currentTrack.trackId,
            "title" to currentTrack.trackName,
            "artist_id" to currentTrack.primaryArtistId,
            "artist_name" to currentTrack.artistNames.firstOrNull(),
            "artists_ids" to listOf(currentTrack.primaryArtistId),
            "album_id" to currentTrack.albumId,
            "album_title" to currentTrack.albumName,
            "duration" to (currentTrack.durationSeconds * 1000).toLong(),
            "track_number" to null,
            "image_id" to currentTrack.imageId,
        )

        val volumeState = player.volumeState.value
        val repeatString = when (player.repeatMode.value) {
            RepeatMode.OFF -> "off"
            RepeatMode.ALL -> "all"
            RepeatMode.ONE -> "one"
        }

        val stateMap = mapOf<String, Any?>(
            "current_track" to currentTrackMap,
            "queue_position" to (queueState.currentIndex),
            "queue_version" to queueVersion.get().toLong(),
            "position" to (player.currentTrackProgressSec.value ?: 0),
            "is_playing" to player.isPlaying.value,
            "volume" to volumeState.volume.toDouble(),
            "muted" to volumeState.isMuted,
            "shuffle" to player.shuffleEnabled.value,
            "repeat" to repeatString,
            "timestamp" to timeProvider.nowUtcMs(),
        )

        webSocketManager.send("$PREFIX.state", stateMap)
    }

    private fun broadcastQueue(tracksIds: List<String>) {
        if (webSocketManager.connectionState.value !is ConnectionState.Connected) return

        val queueMap = mapOf<String, Any?>(
            "queue" to tracksIds.map { trackId ->
                mapOf<String, Any?>(
                    "id" to trackId,
                    "added_at" to timeProvider.nowUtcMs(),
                )
            },
            "queue_version" to queueVersion.incrementAndGet().toLong(),
        )

        webSocketManager.send("$PREFIX.queue_update", queueMap)
        logger.debug("Sent queue update with ${tracksIds.size} tracks")
    }

    private fun sendStoppedState() {
        if (webSocketManager.connectionState.value !is ConnectionState.Connected) return

        val stateMap = mapOf<String, Any?>(
            "current_track" to null,
            "queue_position" to 0,
            "queue_version" to queueVersion.get().toLong(),
            "position" to 0,
            "is_playing" to false,
            "volume" to 1.0,
            "muted" to false,
            "shuffle" to false,
            "repeat" to "off",
            "timestamp" to timeProvider.nowUtcMs(),
        )

        webSocketManager.send("$PREFIX.state", stateMap)
        logger.debug("Sent stopped state")
    }

    private fun handleMessage(type: String, payload: String?) {
        when (type) {
            "$PREFIX.welcome" -> {
                logger.info("Received playback welcome")
                if (player.isActive.value && !isBroadcasting) {
                    startBroadcasting()
                    broadcastState()
                }
            }

            "$PREFIX.command" -> {
                if (payload != null) {
                    handleCommand(payload)
                } else {
                    logger.warn("Received command with no payload")
                }
            }

            "$PREFIX.device_state",
            "$PREFIX.device_queue",
            "$PREFIX.device_stopped",
            "$PREFIX.device_list_changed" -> {
                // Ignored - not displaying other devices in Android UI
            }

            "$PREFIX.error" -> {
                logger.error("Received playback error: $payload")
            }

            else -> {
                logger.debug("Received unknown playback message: $type")
            }
        }
    }

    private fun handleCommand(payloadString: String) {
        try {
            val payloadJson = json.parseToJsonElement(payloadString).jsonObject
            val command = payloadJson["command"]?.jsonPrimitive?.content ?: run {
                logger.warn("Command message missing 'command' field")
                return
            }
            val commandPayload = payloadJson["payload"]

            logger.info("Received command: $command")

            when (command) {
                "play" -> player.setIsPlaying(true)
                "pause" -> player.setIsPlaying(false)
                "next" -> player.skipToNextTrack()
                "prev" -> player.skipToPreviousTrack()
                "seek" -> handleSeekCommand(commandPayload)
                "setVolume" -> handleSetVolumeCommand(commandPayload)
                "setMuted" -> handleSetMutedCommand(commandPayload)
                else -> logger.warn("Unknown command: $command")
            }
        } catch (e: Exception) {
            logger.error("Failed to handle command: $payloadString", e)
        }
    }

    private fun handleSeekCommand(payload: JsonElement?) {
        val position = payload?.jsonObject?.get("position")?.jsonPrimitive?.double ?: run {
            logger.warn("Seek command missing position")
            return
        }
        val duration = player.currentTrackDurationSeconds.value
        if (duration != null && duration > 0) {
            val percentage = (position / duration).toFloat().coerceIn(0f, 1f)
            player.seekToPercentage(percentage)
        }
    }

    private fun handleSetVolumeCommand(payload: JsonElement?) {
        val volume = payload?.jsonObject?.get("volume")?.jsonPrimitive?.double ?: run {
            logger.warn("setVolume command missing volume")
            return
        }
        player.setVolume(volume.toFloat())
    }

    private fun handleSetMutedCommand(payload: JsonElement?) {
        val muted = payload?.jsonObject?.get("muted")?.jsonPrimitive?.boolean ?: run {
            logger.warn("setMuted command missing muted")
            return
        }
        player.setMuted(muted)
    }

    private data class PlayerStateSnapshot(
        val isActive: Boolean,
        val isPlaying: Boolean,
        val trackIndex: Int?,
        val volumeState: VolumeState,
        val shuffleEnabled: Boolean,
        val repeatMode: RepeatMode,
    )

    companion object {
        private const val PREFIX = "playback"
        internal const val BROADCAST_INTERVAL_MS = 5_000L
    }
}
