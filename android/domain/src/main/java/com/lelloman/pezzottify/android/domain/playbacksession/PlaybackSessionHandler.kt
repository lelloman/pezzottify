package com.lelloman.pezzottify.android.domain.playbacksession

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.player.internal.PlaybackMetadataProviderImpl
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import java.util.concurrent.atomic.AtomicInteger
import javax.inject.Inject
import javax.inject.Singleton

@OptIn(DelicateCoroutinesApi::class)
@Singleton
class PlaybackSessionHandler internal constructor(
    private val webSocketManager: WebSocketManager,
    private val player: PlayerImpl,
    private val playbackMetadataProvider: PlaybackMetadataProviderImpl,
    private val deviceInfoProvider: DeviceInfoProvider,
    private val timeProvider: TimeProvider,
    private val playbackModeManager: PlaybackModeManager,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    @Inject
    constructor(
        webSocketManager: WebSocketManager,
        player: PlayerImpl,
        playbackMetadataProvider: PlaybackMetadataProviderImpl,
        deviceInfoProvider: DeviceInfoProvider,
        timeProvider: TimeProvider,
        playbackModeManager: PlaybackModeManager,
        loggerFactory: LoggerFactory,
    ) : this(
        webSocketManager,
        player,
        playbackMetadataProvider,
        deviceInfoProvider,
        timeProvider,
        playbackModeManager,
        GlobalScope,
        loggerFactory,
    )

    private val logger = loggerFactory.getLogger(PlaybackSessionHandler::class)
    private val json = Json { ignoreUnknownKeys = true }

    private var isBroadcasting = false
    private val queueVersion = AtomicInteger(0)
    private var broadcastJob: Job? = null

    private val _myDeviceId = MutableStateFlow<Int?>(null)
    val myDeviceId: StateFlow<Int?> = _myDeviceId.asStateFlow()

    private val _connectedDevices = MutableStateFlow<List<ConnectedDevice>>(emptyList())
    val connectedDevices: StateFlow<List<ConnectedDevice>> = _connectedDevices.asStateFlow()

    private val _otherDeviceStates = MutableStateFlow<Map<Int, RemotePlaybackState>>(emptyMap())
    val otherDeviceStates: StateFlow<Map<Int, RemotePlaybackState>> = _otherDeviceStates.asStateFlow()

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
                    is ConnectionState.Error -> {
                        stopBroadcasting()
                        _myDeviceId.value = null
                        _connectedDevices.value = emptyList()
                        _otherDeviceStates.value = emptyMap()
                    }
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
        if (playbackModeManager.mode.value is PlaybackMode.Remote) return

        val queueState = playbackMetadataProvider.queueState.value ?: return
        val currentTrack = queueState.currentTrack ?: return

        val currentTrackMap = mapOf<String, Any?>(
            "id" to currentTrack.trackId,
            "title" to currentTrack.trackName,
            "artist_id" to currentTrack.primaryArtistId,
            "artist_name" to (currentTrack.artistNames.firstOrNull() ?: ""),
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

        // Compute position as Double (seconds with fractional precision) for consistency
        // with the web client. Use currentTrackPercent and duration as primary source since
        // they're derived from ExoPlayer's actual position in updateProgress().
        val positionSec: Double = run {
            val percent = player.currentTrackPercent.value
            val durationSec = player.currentTrackDurationSeconds.value
            if (percent != null && durationSec != null && durationSec > 0) {
                (percent.toDouble() / 100.0) * durationSec
            } else {
                player.currentTrackProgressSec.value?.toDouble() ?: 0.0
            }
        }

        val stateMap = mapOf<String, Any?>(
            "current_track" to currentTrackMap,
            "queue_position" to (queueState.currentIndex),
            "queue_version" to queueVersion.get().toLong(),
            "position" to positionSec,
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
        if (playbackModeManager.mode.value is PlaybackMode.Remote) return

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
                if (payload != null) {
                    handleWelcome(payload)
                }
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

            "$PREFIX.device_state" -> {
                if (payload != null) handleDeviceState(payload)
            }

            "$PREFIX.device_queue" -> {
                // Stored for Phase 3 remote mode
                logger.debug("Received device queue update")
            }

            "$PREFIX.device_stopped" -> {
                if (payload != null) handleDeviceStopped(payload)
            }

            "$PREFIX.device_list_changed" -> {
                if (payload != null) handleDeviceListChanged(payload)
            }

            "$PREFIX.error" -> {
                logger.error("Received playback error: $payload")
            }

            else -> {
                logger.debug("Received unknown playback message: $type")
            }
        }
    }

    private fun handleWelcome(payloadString: String) {
        try {
            val payloadJson = json.parseToJsonElement(payloadString).jsonObject

            _myDeviceId.value = payloadJson["device_id"]?.jsonPrimitive?.int

            val devicesArray = payloadJson["devices"]?.jsonArray
            if (devicesArray != null) {
                _connectedDevices.value = devicesArray.map { element ->
                    val obj = element.jsonObject
                    ConnectedDevice(
                        id = obj["id"]?.jsonPrimitive?.int ?: 0,
                        name = obj["name"]?.jsonPrimitive?.content ?: "",
                        deviceType = obj["device_type"]?.jsonPrimitive?.content ?: "unknown",
                    )
                }
            }

            val activeDevices = payloadJson["session"]?.jsonObject?.get("active_devices")?.jsonArray
            if (activeDevices != null) {
                val states = mutableMapOf<Int, RemotePlaybackState>()
                for (device in activeDevices) {
                    val deviceObj = device.jsonObject
                    val deviceId = deviceObj["device_id"]?.jsonPrimitive?.int ?: continue
                    if (deviceId == _myDeviceId.value) continue
                    val stateObj = deviceObj["state"]?.jsonObject ?: continue
                    val parsed = parseRemotePlaybackState(stateObj)
                    if (parsed != null) {
                        states[deviceId] = parsed.copy(receivedAt = System.currentTimeMillis())
                    }
                }
                _otherDeviceStates.value = states
            }
        } catch (e: Exception) {
            logger.error("Failed to parse welcome payload", e)
        }
    }

    private fun handleDeviceState(payloadString: String) {
        try {
            val payloadJson = json.parseToJsonElement(payloadString).jsonObject
            val deviceId = payloadJson["device_id"]?.jsonPrimitive?.int ?: return
            if (deviceId == _myDeviceId.value) return

            val stateObj = payloadJson["state"]?.jsonObject ?: return
            val parsed = parseRemotePlaybackState(stateObj) ?: return

            _otherDeviceStates.value = _otherDeviceStates.value + (deviceId to parsed.copy(receivedAt = System.currentTimeMillis()))
        } catch (e: Exception) {
            logger.error("Failed to parse device state", e)
        }
    }

    private fun handleDeviceStopped(payloadString: String) {
        try {
            val payloadJson = json.parseToJsonElement(payloadString).jsonObject
            val deviceId = payloadJson["device_id"]?.jsonPrimitive?.int ?: return
            _otherDeviceStates.value = _otherDeviceStates.value - deviceId
        } catch (e: Exception) {
            logger.error("Failed to parse device stopped", e)
        }
    }

    private fun handleDeviceListChanged(payloadString: String) {
        try {
            val payloadJson = json.parseToJsonElement(payloadString).jsonObject
            val devicesArray = payloadJson["devices"]?.jsonArray ?: return
            _connectedDevices.value = devicesArray.map { element ->
                val obj = element.jsonObject
                ConnectedDevice(
                    id = obj["id"]?.jsonPrimitive?.int ?: 0,
                    name = obj["name"]?.jsonPrimitive?.content ?: "",
                    deviceType = obj["device_type"]?.jsonPrimitive?.content ?: "unknown",
                )
            }
        } catch (e: Exception) {
            logger.error("Failed to parse device list changed", e)
        }
    }

    private fun parseRemotePlaybackState(stateObj: Map<String, JsonElement>): RemotePlaybackState? {
        return try {
            val currentTrackObj = stateObj["current_track"]?.jsonObject
            val currentTrack = if (currentTrackObj != null) {
                RemoteTrackInfo(
                    id = currentTrackObj["id"]?.jsonPrimitive?.content ?: return null,
                    title = currentTrackObj["title"]?.jsonPrimitive?.content ?: "",
                    artistName = currentTrackObj["artist_name"]?.jsonPrimitive?.content,
                    albumTitle = currentTrackObj["album_title"]?.jsonPrimitive?.content,
                    durationMs = currentTrackObj["duration"]?.jsonPrimitive?.double?.let { dur ->
                        // Web sends seconds (e.g. 133.8), Android sends ms (e.g. 133800)
                        if (dur > 1000) dur.toLong() else (dur * 1000).toLong()
                    } ?: 0L,
                    imageId = currentTrackObj["image_id"]?.jsonPrimitive?.content,
                )
            } else null

            RemotePlaybackState(
                currentTrack = currentTrack,
                position = stateObj["position"]?.jsonPrimitive?.double ?: 0.0,
                isPlaying = stateObj["is_playing"]?.jsonPrimitive?.boolean ?: false,
                volume = stateObj["volume"]?.jsonPrimitive?.double?.toFloat() ?: 1.0f,
                muted = stateObj["muted"]?.jsonPrimitive?.boolean ?: false,
                shuffle = stateObj["shuffle"]?.jsonPrimitive?.boolean ?: false,
                repeat = stateObj["repeat"]?.jsonPrimitive?.content ?: "off",
                timestamp = stateObj["timestamp"]?.jsonPrimitive?.long ?: 0L,
            )
        } catch (e: Exception) {
            logger.error("Failed to parse remote playback state", e)
            null
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

            // Dispatch to main thread — MediaController requires it
            scope.launch(Dispatchers.Main) {
                executeCommand(command, commandPayload)
                // Broadcast updated state immediately so the controlling device sees the change
                broadcastState()
            }
        } catch (e: Exception) {
            logger.error("Failed to handle command: $payloadString", e)
        }
    }

    private fun executeCommand(command: String, commandPayload: JsonElement?) {
        when (command) {
            "play" -> player.setIsPlaying(true)
            "pause" -> player.setIsPlaying(false)
            "next" -> player.skipToNextTrack()
            "prev" -> player.skipToPreviousTrack()
            "seek" -> handleSeekCommand(commandPayload)
            "setVolume" -> handleSetVolumeCommand(commandPayload)
            "setMuted" -> handleSetMutedCommand(commandPayload)
            "loadAlbum" -> handleLoadAlbumCommand(commandPayload)
            "loadPlaylist" -> handleLoadPlaylistCommand(commandPayload)
            "loadSingleTrack" -> handleLoadSingleTrackCommand(commandPayload)
            "addAlbumToQueue" -> handleAddAlbumToQueueCommand(commandPayload)
            "addPlaylistToQueue" -> handleAddPlaylistToQueueCommand(commandPayload)
            "addTracksToQueue" -> handleAddTracksToQueueCommand(commandPayload)
            "skipToTrack" -> handleSkipToTrackCommand(commandPayload)
            "setShuffle" -> handleSetShuffleCommand(commandPayload)
            "setRepeat" -> handleSetRepeatCommand(commandPayload)
            "removeTrack" -> handleRemoveTrackCommand(commandPayload)
            "moveTrack" -> handleMoveTrackCommand(commandPayload)
            else -> logger.warn("Unknown command: $command")
        }
    }

    private fun handleSeekCommand(payload: JsonElement?) {
        val position = payload?.jsonObject?.get("position")?.jsonPrimitive?.double ?: run {
            logger.warn("Seek command missing position")
            return
        }
        val duration = player.currentTrackDurationSeconds.value
        if (duration != null && duration > 0) {
            // seekToPercentage expects 0-100 range
            val percentage = ((position / duration) * 100.0).toFloat().coerceIn(0f, 100f)
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

    private fun handleLoadAlbumCommand(payload: JsonElement?) {
        val obj = payload?.jsonObject ?: return
        val albumId = obj["albumId"]?.jsonPrimitive?.content ?: run {
            logger.warn("loadAlbum command missing albumId")
            return
        }
        val startTrackId = obj["startTrackId"]?.jsonPrimitive?.content
        player.loadAlbum(albumId, startTrackId)
    }

    private fun handleLoadPlaylistCommand(payload: JsonElement?) {
        val obj = payload?.jsonObject ?: return
        val playlistId = obj["playlistId"]?.jsonPrimitive?.content ?: run {
            logger.warn("loadPlaylist command missing playlistId")
            return
        }
        val startTrackId = obj["startTrackId"]?.jsonPrimitive?.content
        player.loadUserPlaylist(playlistId, startTrackId)
    }

    private fun handleLoadSingleTrackCommand(payload: JsonElement?) {
        val trackId = payload?.jsonObject?.get("trackId")?.jsonPrimitive?.content ?: run {
            logger.warn("loadSingleTrack command missing trackId")
            return
        }
        player.loadSingleTrack(trackId)
    }

    private fun handleAddAlbumToQueueCommand(payload: JsonElement?) {
        val albumId = payload?.jsonObject?.get("albumId")?.jsonPrimitive?.content ?: run {
            logger.warn("addAlbumToQueue command missing albumId")
            return
        }
        player.addAlbumToPlaylist(albumId)
    }

    private fun handleAddPlaylistToQueueCommand(payload: JsonElement?) {
        val playlistId = payload?.jsonObject?.get("playlistId")?.jsonPrimitive?.content ?: run {
            logger.warn("addPlaylistToQueue command missing playlistId")
            return
        }
        player.addUserPlaylistToQueue(playlistId)
    }

    private fun handleAddTracksToQueueCommand(payload: JsonElement?) {
        val trackIds = payload?.jsonObject?.get("trackIds")?.jsonArray?.map {
            it.jsonPrimitive.content
        } ?: run {
            logger.warn("addTracksToQueue command missing trackIds")
            return
        }
        player.addTracksToPlaylist(trackIds)
    }

    private fun handleSkipToTrackCommand(payload: JsonElement?) {
        val index = payload?.jsonObject?.get("index")?.jsonPrimitive?.int ?: run {
            logger.warn("skipToTrack command missing index")
            return
        }
        player.loadTrackIndex(index)
    }

    private fun handleSetShuffleCommand(payload: JsonElement?) {
        val enabled = payload?.jsonObject?.get("enabled")?.jsonPrimitive?.boolean ?: run {
            logger.warn("setShuffle command missing enabled")
            return
        }
        // Toggle if current state differs from requested
        if (player.shuffleEnabled.value != enabled) {
            player.toggleShuffle()
        }
    }

    private fun handleSetRepeatCommand(payload: JsonElement?) {
        val mode = payload?.jsonObject?.get("mode")?.jsonPrimitive?.content ?: run {
            logger.warn("setRepeat command missing mode")
            return
        }
        val targetMode = when (mode) {
            "all" -> RepeatMode.ALL
            "one" -> RepeatMode.ONE
            else -> RepeatMode.OFF
        }
        // Cycle until we reach the target mode
        while (player.repeatMode.value != targetMode) {
            player.cycleRepeatMode()
        }
    }

    private fun handleRemoveTrackCommand(payload: JsonElement?) {
        val trackId = payload?.jsonObject?.get("trackId")?.jsonPrimitive?.content
        val index = payload?.jsonObject?.get("index")?.jsonPrimitive?.int
        if (trackId != null) {
            player.removeTrackFromPlaylist(trackId)
        } else if (index != null) {
            // Lookup trackId from playlist by index
            val playlist = player.playbackPlaylist.value
            val trackIdAtIndex = playlist?.tracksIds?.getOrNull(index)
            if (trackIdAtIndex != null) {
                player.removeTrackFromPlaylist(trackIdAtIndex)
            } else {
                logger.warn("removeTrack: index $index out of bounds")
            }
        } else {
            logger.warn("removeTrack command missing trackId or index")
        }
    }

    private fun handleMoveTrackCommand(payload: JsonElement?) {
        val obj = payload?.jsonObject ?: return
        val fromIndex = obj["fromIndex"]?.jsonPrimitive?.int ?: run {
            logger.warn("moveTrack command missing fromIndex")
            return
        }
        val toIndex = obj["toIndex"]?.jsonPrimitive?.int ?: run {
            logger.warn("moveTrack command missing toIndex")
            return
        }
        player.moveTrack(fromIndex, toIndex)
    }

    private data class PlayerStateSnapshot(
        val isActive: Boolean,
        val isPlaying: Boolean,
        val trackIndex: Int?,
        val volumeState: VolumeState,
        val shuffleEnabled: Boolean,
        val repeatMode: RepeatMode,
    )

    fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int) {
        val msg = mapOf<String, Any?>(
            "command" to command,
            "payload" to payload,
            "target_device_id" to targetDeviceId,
        )
        webSocketManager.send("$PREFIX.command", msg)
        logger.debug("Sent command '$command' to device $targetDeviceId")
    }

    companion object {
        private const val PREFIX = "playback"
        internal const val BROADCAST_INTERVAL_MS = 5_000L
    }
}
