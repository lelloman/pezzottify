package com.lelloman.pezzottify.android.domain.playbacksession

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.internal.PlaybackMetadataProviderImpl
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.MessageHandler
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackSessionHandlerTest {

    private lateinit var webSocketManager: WebSocketManager
    private lateinit var player: PlayerImpl
    private lateinit var playbackMetadataProvider: PlaybackMetadataProviderImpl
    private lateinit var deviceInfoProvider: DeviceInfoProvider
    private lateinit var timeProvider: TimeProvider
    private lateinit var playbackModeManager: PlaybackModeManager
    private lateinit var loggerFactory: LoggerFactory

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var handler: PlaybackSessionHandler

    // Flow controls
    private lateinit var connectionStateFlow: MutableStateFlow<ConnectionState>
    private lateinit var isActiveFlow: MutableStateFlow<Boolean>
    private lateinit var isPlayingFlow: MutableStateFlow<Boolean>
    private lateinit var currentTrackIndexFlow: MutableStateFlow<Int?>
    private lateinit var volumeStateFlow: MutableStateFlow<VolumeState>
    private lateinit var shuffleEnabledFlow: MutableStateFlow<Boolean>
    private lateinit var repeatModeFlow: MutableStateFlow<RepeatMode>
    private lateinit var playbackPlaylistFlow: MutableStateFlow<PlaybackPlaylist?>
    private lateinit var queueStateFlow: MutableStateFlow<PlaybackQueueState?>
    private lateinit var currentTrackProgressSecFlow: MutableStateFlow<Int?>
    private lateinit var currentTrackDurationSecondsFlow: MutableStateFlow<Int?>
    private lateinit var currentTrackPercentFlow: MutableStateFlow<Float?>
    // Captured handler
    private lateinit var capturedMessageHandler: MessageHandler

    private var currentTime = 1_000_000L

    // Captured send calls
    private val sentMessages = mutableListOf<Pair<String, Any?>>()

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)

        connectionStateFlow = MutableStateFlow(ConnectionState.Disconnected)
        isActiveFlow = MutableStateFlow(false)
        isPlayingFlow = MutableStateFlow(false)
        currentTrackIndexFlow = MutableStateFlow(null)
        volumeStateFlow = MutableStateFlow(VolumeState(volume = 1.0f, isMuted = false))
        shuffleEnabledFlow = MutableStateFlow(false)
        repeatModeFlow = MutableStateFlow(RepeatMode.OFF)
        playbackPlaylistFlow = MutableStateFlow(null)
        queueStateFlow = MutableStateFlow(null)
        currentTrackProgressSecFlow = MutableStateFlow(null)
        currentTrackDurationSecondsFlow = MutableStateFlow(null)
        currentTrackPercentFlow = MutableStateFlow(null)

        val handlerSlot = slot<MessageHandler>()
        webSocketManager = mockk(relaxed = true) {
            every { connectionState } returns connectionStateFlow
            every { registerHandler(any(), capture(handlerSlot)) } answers {
                capturedMessageHandler = handlerSlot.captured
            }
            every { send(any(), any()) } answers {
                sentMessages.add(firstArg<String>() to secondArg())
            }
        }

        player = mockk(relaxed = true) {
            every { isActive } returns isActiveFlow
            every { isPlaying } returns isPlayingFlow
            every { currentTrackIndex } returns currentTrackIndexFlow
            every { volumeState } returns volumeStateFlow
            every { shuffleEnabled } returns shuffleEnabledFlow
            every { repeatMode } returns repeatModeFlow
            every { playbackPlaylist } returns playbackPlaylistFlow
            every { currentTrackProgressSec } returns currentTrackProgressSecFlow
            every { currentTrackDurationSeconds } returns currentTrackDurationSecondsFlow
            every { currentTrackPercent } returns currentTrackPercentFlow
        }

        playbackMetadataProvider = mockk(relaxed = true) {
            every { queueState } returns queueStateFlow
        }

        deviceInfoProvider = mockk {
            every { getDeviceInfo() } returns DeviceInfo(
                deviceUuid = "test-uuid",
                deviceType = "android",
                deviceName = "Test Device",
                osInfo = "Android 14",
            )
        }

        timeProvider = TimeProvider { currentTime }

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk {
            every { getLogger(any<String>()) } returns mockLogger
            every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns mockLogger
            every { getValue(any(), any()) } returns mockLogger
        }

        playbackModeManager = PlaybackModeManager(loggerFactory)

        sentMessages.clear()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createHandler(scope: CoroutineScope) = PlaybackSessionHandler(
        webSocketManager = webSocketManager,
        player = player,
        playbackMetadataProvider = playbackMetadataProvider,
        deviceInfoProvider = deviceInfoProvider,
        timeProvider = timeProvider,
        playbackModeManager = playbackModeManager,
        scope = scope,
        loggerFactory = loggerFactory,
    )

    private fun setUpActivePlayer() {
        queueStateFlow.value = PlaybackQueueState(
            tracks = listOf(TEST_TRACK_METADATA),
            currentIndex = 0,
        )
        playbackPlaylistFlow.value = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Album("album-1"),
            tracksIds = listOf("track-1"),
        )
        currentTrackProgressSecFlow.value = 42
        currentTrackDurationSecondsFlow.value = 300
        currentTrackPercentFlow.value = 14.0f
        isActiveFlow.value = true
        isPlayingFlow.value = true
        currentTrackIndexFlow.value = 0
    }

    // ========== Hello Tests ==========

    @Test
    fun `sendHello sent on WebSocket connect with correct device info`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        val helloMessage = sentMessages.find { it.first == "playback.hello" }
        assertThat(helloMessage).isNotNull()
        @Suppress("UNCHECKED_CAST")
        val payload = helloMessage!!.second as Map<String, Any?>
        assertThat(payload["device_name"]).isEqualTo("Test Device")
        assertThat(payload["device_type"]).isEqualTo("android")
    }

    @Test
    fun `hello resent on reconnect`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        // First connect
        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        val firstHelloCount = sentMessages.count { it.first == "playback.hello" }
        assertThat(firstHelloCount).isEqualTo(1)

        // Disconnect
        connectionStateFlow.value = ConnectionState.Disconnected
        testScheduler.runCurrent()

        // Reconnect
        connectionStateFlow.value = ConnectionState.Connected(deviceId = 2, serverVersion = "1.0")
        testScheduler.runCurrent()

        val totalHelloCount = sentMessages.count { it.first == "playback.hello" }
        assertThat(totalHelloCount).isEqualTo(2)
    }

    // ========== Broadcast State Tests ==========

    @Test
    fun `broadcastState sent when player becomes active`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        sentMessages.clear()
        setUpActivePlayer()
        testScheduler.runCurrent()

        val stateMessages = sentMessages.filter { it.first == "playback.state" }
        assertThat(stateMessages).isNotEmpty()
    }

    @Test
    fun `broadcastState contains correct nested payload`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        sentMessages.clear()
        setUpActivePlayer()
        testScheduler.runCurrent()

        val stateMessage = sentMessages.find { it.first == "playback.state" }
        assertThat(stateMessage).isNotNull()

        @Suppress("UNCHECKED_CAST")
        val payload = stateMessage!!.second as Map<String, Any?>
        assertThat(payload["is_playing"]).isEqualTo(true)
        assertThat(payload["position"] as Double).isWithin(0.01).of(42.0)
        assertThat(payload["volume"]).isEqualTo(1.0)
        assertThat(payload["muted"]).isEqualTo(false)
        assertThat(payload["shuffle"]).isEqualTo(false)
        assertThat(payload["repeat"]).isEqualTo("off")
        assertThat(payload["queue_position"]).isEqualTo(0)
        assertThat(payload["timestamp"]).isEqualTo(currentTime)

        @Suppress("UNCHECKED_CAST")
        val currentTrack = payload["current_track"] as Map<String, Any?>
        assertThat(currentTrack["id"]).isEqualTo("track-1")
        assertThat(currentTrack["title"]).isEqualTo("Test Track")
        assertThat(currentTrack["artist_id"]).isEqualTo("artist-1")
        assertThat(currentTrack["artist_name"]).isEqualTo("Test Artist")
        assertThat(currentTrack["album_id"]).isEqualTo("album-1")
        assertThat(currentTrack["album_title"]).isEqualTo("Test Album")
        assertThat(currentTrack["duration"]).isEqualTo(300_000L)
        assertThat(currentTrack["image_id"]).isEqualTo("image-1")
    }

    @Test
    fun `broadcastState sends repeat mode correctly`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        // Change repeat mode to ALL
        sentMessages.clear()
        repeatModeFlow.value = RepeatMode.ALL
        testScheduler.runCurrent()

        var stateMessage = sentMessages.find { it.first == "playback.state" }
        @Suppress("UNCHECKED_CAST")
        assertThat((stateMessage!!.second as Map<String, Any?>)["repeat"]).isEqualTo("all")

        // Change to ONE
        sentMessages.clear()
        repeatModeFlow.value = RepeatMode.ONE
        testScheduler.runCurrent()

        stateMessage = sentMessages.find { it.first == "playback.state" }
        @Suppress("UNCHECKED_CAST")
        assertThat((stateMessage!!.second as Map<String, Any?>)["repeat"]).isEqualTo("one")
    }

    // ========== Queue Broadcast Tests ==========

    @Test
    fun `broadcastQueue sent when playlist changes`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        sentMessages.clear()

        // Change playlist
        playbackPlaylistFlow.value = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Album("album-2"),
            tracksIds = listOf("track-2", "track-3"),
        )
        testScheduler.runCurrent()

        val queueMessage = sentMessages.find { it.first == "playback.queue_update" }
        assertThat(queueMessage).isNotNull()

        @Suppress("UNCHECKED_CAST")
        val payload = queueMessage!!.second as Map<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val queue = payload["queue"] as List<Map<String, Any?>>
        assertThat(queue).hasSize(2)
        assertThat(queue[0]["id"]).isEqualTo("track-2")
        assertThat(queue[1]["id"]).isEqualTo("track-3")
        assertThat(payload["queue_version"]).isNotNull()
    }

    // ========== Periodic Broadcast Tests ==========

    @Test
    fun `periodic broadcast fires every 5 seconds`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        val initialStateCount = sentMessages.count { it.first == "playback.state" }

        advanceTimeBy(5_500)

        val afterOneInterval = sentMessages.count { it.first == "playback.state" }
        assertThat(afterOneInterval).isGreaterThan(initialStateCount)

        advanceTimeBy(5_500)

        val afterTwoIntervals = sentMessages.count { it.first == "playback.state" }
        assertThat(afterTwoIntervals).isGreaterThan(afterOneInterval)
    }

    // ========== Command Tests ==========

    @Test
    fun `command play calls setIsPlaying true`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"play"}"""
        )
        testScheduler.runCurrent()

        verify { player.setIsPlaying(true) }
    }

    @Test
    fun `command pause calls setIsPlaying false`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"pause"}"""
        )
        testScheduler.runCurrent()

        verify { player.setIsPlaying(false) }
    }

    @Test
    fun `command next calls skipToNextTrack`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"next"}"""
        )
        testScheduler.runCurrent()

        verify { player.skipToNextTrack() }
    }

    @Test
    fun `command prev calls skipToPreviousTrack`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"prev"}"""
        )
        testScheduler.runCurrent()

        verify { player.skipToPreviousTrack() }
    }

    @Test
    fun `command seek computes correct percentage`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        currentTrackDurationSecondsFlow.value = 200

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"seek","payload":{"position":100.0}}"""
        )
        testScheduler.runCurrent()

        verify { player.seekToPercentage(50.0f) }
    }

    @Test
    fun `command setVolume calls player setVolume`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"setVolume","payload":{"volume":0.75}}"""
        )
        testScheduler.runCurrent()

        verify { player.setVolume(0.75f) }
    }

    @Test
    fun `command setMuted calls player setMuted`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"setMuted","payload":{"muted":true}}"""
        )
        testScheduler.runCurrent()

        verify { player.setMuted(true) }
    }

    // ========== Stopped State Tests ==========

    @Test
    fun `stopped state sent when player becomes inactive`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        sentMessages.clear()

        // Player becomes inactive
        isActiveFlow.value = false
        testScheduler.runCurrent()

        val stateMessage = sentMessages.find { it.first == "playback.state" }
        assertThat(stateMessage).isNotNull()

        @Suppress("UNCHECKED_CAST")
        val payload = stateMessage!!.second as Map<String, Any?>
        assertThat(payload["current_track"]).isNull()
        assertThat(payload["is_playing"]).isEqualTo(false)
    }

    // ========== Disconnect Tests ==========

    @Test
    fun `broadcasting stops on WebSocket disconnect`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        // Disconnect
        connectionStateFlow.value = ConnectionState.Disconnected
        testScheduler.runCurrent()

        sentMessages.clear()

        // Advance time - no periodic broadcasts should fire
        advanceTimeBy(15_000)

        val stateMessages = sentMessages.filter { it.first == "playback.state" }
        assertThat(stateMessages).isEmpty()
    }

    @Test
    fun `no queue broadcast when not broadcasting`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        // Not connected, player not active
        sentMessages.clear()

        playbackPlaylistFlow.value = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Album("album-1"),
            tracksIds = listOf("track-1"),
        )
        testScheduler.runCurrent()

        val queueMessages = sentMessages.filter { it.first == "playback.queue_update" }
        assertThat(queueMessages).isEmpty()
    }

    // ========== Welcome Message Tests ==========

    @Test
    fun `welcome message starts broadcasting when player is active`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        setUpActivePlayer()
        testScheduler.runCurrent()

        // Disconnect stops broadcasting
        connectionStateFlow.value = ConnectionState.Disconnected
        testScheduler.runCurrent()

        // Reconnect
        connectionStateFlow.value = ConnectionState.Connected(deviceId = 2, serverVersion = "1.0")
        testScheduler.runCurrent()

        sentMessages.clear()

        // Receive welcome - should trigger broadcast since player is active
        capturedMessageHandler.onMessage("playback.welcome", null)
        testScheduler.runCurrent()

        // Periodic broadcast should fire after welcome started broadcasting
        advanceTimeBy(5_500)

        val stateMessages = sentMessages.filter { it.first == "playback.state" }
        assertThat(stateMessages).isNotEmpty()
    }

    // ========== Edge Case Tests ==========

    @Test
    fun `seek command does nothing when duration is null`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        currentTrackDurationSecondsFlow.value = null

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"seek","payload":{"position":100.0}}"""
        )

        verify(exactly = 0) { player.seekToPercentage(any()) }
    }

    @Test
    fun `seek command does nothing when duration is zero`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        currentTrackDurationSecondsFlow.value = 0

        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"seek","payload":{"position":100.0}}"""
        )

        verify(exactly = 0) { player.seekToPercentage(any()) }
    }

    @Test
    fun `malformed command payload does not crash`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        // Invalid JSON
        capturedMessageHandler.onMessage("playback.command", "not valid json")

        // Missing command field
        capturedMessageHandler.onMessage("playback.command", """{"foo":"bar"}""")

        // Seek with missing position
        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"seek","payload":{}}"""
        )

        // setVolume with missing volume
        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"setVolume","payload":{}}"""
        )

        // setMuted with missing muted
        capturedMessageHandler.onMessage(
            "playback.command",
            """{"command":"setMuted","payload":{}}"""
        )

        // None of these should cause a crash - if we get here, the test passes
    }

    // ========== Welcome Device State Tests ==========

    @Test
    fun `welcome message populates myDeviceId`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":42,"devices":[],"session":{"active_devices":[]}}"""
        )

        assertThat(handler.myDeviceId.value).isEqualTo(42)
    }

    @Test
    fun `welcome message populates connectedDevices`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[{"id":1,"name":"Android","device_type":"android"},{"id":2,"name":"Chrome on Linux","device_type":"web"}],"session":{"active_devices":[]}}"""
        )

        assertThat(handler.connectedDevices.value).hasSize(2)
        assertThat(handler.connectedDevices.value[0].id).isEqualTo(1)
        assertThat(handler.connectedDevices.value[0].name).isEqualTo("Android")
        assertThat(handler.connectedDevices.value[0].deviceType).isEqualTo("android")
        assertThat(handler.connectedDevices.value[1].id).isEqualTo(2)
        assertThat(handler.connectedDevices.value[1].name).isEqualTo("Chrome on Linux")
        assertThat(handler.connectedDevices.value[1].deviceType).isEqualTo("web")
    }

    @Test
    fun `welcome message populates otherDeviceStates from active_devices`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[{"id":1,"name":"Android","device_type":"android"},{"id":2,"name":"Web","device_type":"web"}],"session":{"active_devices":[{"device_id":2,"device_name":"Web","state":{"current_track":{"id":"t1","title":"Song","artist_name":"Artist","album_title":"Album","duration":180000,"image_id":"img1"},"position":45.0,"is_playing":true,"volume":0.8,"muted":false,"shuffle":false,"repeat":"off","timestamp":1000}}]}}"""
        )

        assertThat(handler.otherDeviceStates.value).hasSize(1)
        val state = handler.otherDeviceStates.value[2]!!
        assertThat(state.isPlaying).isTrue()
        assertThat(state.position).isEqualTo(45.0)
        assertThat(state.currentTrack!!.id).isEqualTo("t1")
        assertThat(state.currentTrack!!.title).isEqualTo("Song")
        assertThat(state.currentTrack!!.artistName).isEqualTo("Artist")
        assertThat(state.currentTrack!!.durationMs).isEqualTo(180000L)
        assertThat(state.currentTrack!!.imageId).isEqualTo("img1")
    }

    @Test
    fun `welcome message excludes own device from otherDeviceStates`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[],"session":{"active_devices":[{"device_id":1,"device_name":"Self","state":{"current_track":{"id":"t1","title":"Song","artist_name":"A","duration":1000},"position":0,"is_playing":false,"volume":1,"muted":false,"shuffle":false,"repeat":"off","timestamp":0}}]}}"""
        )

        assertThat(handler.otherDeviceStates.value).isEmpty()
    }

    // ========== Device State Update Tests ==========

    @Test
    fun `device_state updates otherDeviceStates`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        // Set myDeviceId first via welcome
        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[],"session":{"active_devices":[]}}"""
        )

        capturedMessageHandler.onMessage(
            "playback.device_state",
            """{"device_id":3,"device_name":"Phone","state":{"current_track":{"id":"t2","title":"Track 2","artist_name":"Art","duration":200000,"image_id":"i2"},"position":10.5,"is_playing":true,"volume":1.0,"muted":false,"shuffle":true,"repeat":"all","timestamp":2000}}"""
        )

        assertThat(handler.otherDeviceStates.value).hasSize(1)
        val state = handler.otherDeviceStates.value[3]!!
        assertThat(state.currentTrack!!.title).isEqualTo("Track 2")
        assertThat(state.position).isEqualTo(10.5)
        assertThat(state.shuffle).isTrue()
        assertThat(state.repeat).isEqualTo("all")
        assertThat(state.receivedAt).isGreaterThan(0L)
    }

    @Test
    fun `device_state ignores own device updates`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[],"session":{"active_devices":[]}}"""
        )

        capturedMessageHandler.onMessage(
            "playback.device_state",
            """{"device_id":1,"device_name":"Self","state":{"current_track":{"id":"t1","title":"S","duration":1000},"position":0,"is_playing":false,"volume":1,"muted":false,"shuffle":false,"repeat":"off","timestamp":0}}"""
        )

        assertThat(handler.otherDeviceStates.value).isEmpty()
    }

    // ========== Device Stopped Tests ==========

    @Test
    fun `device_stopped removes device from otherDeviceStates`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[],"session":{"active_devices":[{"device_id":5,"device_name":"W","state":{"current_track":{"id":"t","title":"T","duration":1000},"position":0,"is_playing":false,"volume":1,"muted":false,"shuffle":false,"repeat":"off","timestamp":0}}]}}"""
        )

        assertThat(handler.otherDeviceStates.value).hasSize(1)

        capturedMessageHandler.onMessage(
            "playback.device_stopped",
            """{"device_id":5}"""
        )

        assertThat(handler.otherDeviceStates.value).isEmpty()
    }

    // ========== Device List Changed Tests ==========

    @Test
    fun `device_list_changed updates connectedDevices`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[{"id":1,"name":"A","device_type":"android"}],"session":{"active_devices":[]}}"""
        )

        assertThat(handler.connectedDevices.value).hasSize(1)

        capturedMessageHandler.onMessage(
            "playback.device_list_changed",
            """{"devices":[{"id":1,"name":"A","device_type":"android"},{"id":2,"name":"B","device_type":"web"},{"id":3,"name":"C","device_type":"web"}]}"""
        )

        assertThat(handler.connectedDevices.value).hasSize(3)
        assertThat(handler.connectedDevices.value[2].name).isEqualTo("C")
    }

    // ========== Disconnect State Reset Tests ==========

    @Test
    fun `disconnect resets device state`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        connectionStateFlow.value = ConnectionState.Connected(deviceId = 1, serverVersion = "1.0")
        testScheduler.runCurrent()

        capturedMessageHandler.onMessage(
            "playback.welcome",
            """{"device_id":1,"devices":[{"id":1,"name":"A","device_type":"android"}],"session":{"active_devices":[{"device_id":2,"device_name":"B","state":{"current_track":{"id":"t","title":"T","duration":1000},"position":0,"is_playing":false,"volume":1,"muted":false,"shuffle":false,"repeat":"off","timestamp":0}}]}}"""
        )

        assertThat(handler.myDeviceId.value).isEqualTo(1)
        assertThat(handler.connectedDevices.value).isNotEmpty()
        assertThat(handler.otherDeviceStates.value).isNotEmpty()

        connectionStateFlow.value = ConnectionState.Disconnected
        testScheduler.runCurrent()

        assertThat(handler.myDeviceId.value).isNull()
        assertThat(handler.connectedDevices.value).isEmpty()
        assertThat(handler.otherDeviceStates.value).isEmpty()
    }

    // ========== Send Command Tests ==========

    @Test
    fun `sendCommand sends correctly formatted message`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        handler.sendCommand("play", emptyMap(), 42)

        val commandMessage = sentMessages.find { it.first == "playback.command" }
        assertThat(commandMessage).isNotNull()
        @Suppress("UNCHECKED_CAST")
        val payload = commandMessage!!.second as Map<String, Any?>
        assertThat(payload["command"]).isEqualTo("play")
        assertThat(payload["target_device_id"]).isEqualTo(42)
    }

    @Test
    fun `sendCommand includes payload`() = runTest {
        handler = createHandler(backgroundScope)
        handler.initialize()
        testScheduler.runCurrent()

        handler.sendCommand("seek", mapOf("position" to 45.0), 7)

        val commandMessage = sentMessages.find { it.first == "playback.command" }
        @Suppress("UNCHECKED_CAST")
        val payload = commandMessage!!.second as Map<String, Any?>
        assertThat(payload["command"]).isEqualTo("seek")
        assertThat(payload["target_device_id"]).isEqualTo(7)
        @Suppress("UNCHECKED_CAST")
        val cmdPayload = payload["payload"] as Map<String, Any?>
        assertThat(cmdPayload["position"]).isEqualTo(45.0)
    }

    companion object {
        private val TEST_TRACK_METADATA = TrackMetadata(
            trackId = "track-1",
            trackName = "Test Track",
            artistNames = listOf("Test Artist"),
            primaryArtistId = "artist-1",
            albumId = "album-1",
            albumName = "Test Album",
            artworkUrl = "https://example.com/image.jpg",
            imageId = "image-1",
            durationSeconds = 300,
        )
    }
}
