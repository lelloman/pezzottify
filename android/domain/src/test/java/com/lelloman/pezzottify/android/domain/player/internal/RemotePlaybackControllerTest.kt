package com.lelloman.pezzottify.android.domain.player.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.playbacksession.ConnectedDevice
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState
import com.lelloman.pezzottify.android.domain.playbacksession.RemoteTrackInfo
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RemotePlaybackControllerTest {

    private val dispatcher = StandardTestDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private var nowMs = 1_000_000L
    private val deviceId = 7
    private val states = MutableStateFlow<Map<Int, RemotePlaybackState>>(emptyMap())
    private val devices = MutableStateFlow<List<ConnectedDevice>>(emptyList())
    private val listFresh = MutableStateFlow(false)
    private val sessionHandler = mockk<PlaybackSessionHandler>(relaxed = true) {
        every { otherDeviceStates } returns states
        every { connectedDevices } returns devices
        every { deviceListFresh } returns listFresh
    }
    private val loggerFactory = mockk<LoggerFactory> {
        val logger = mockk<Logger>(relaxed = true)
        every { getLogger(any<String>()) } returns logger
        every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns logger
        every { getValue(any(), any()) } returns logger
    }
    private val modeManager = PlaybackModeManager(loggerFactory)
    private val controller = RemotePlaybackController(sessionHandler, modeManager, scope, { nowMs }, loggerFactory)

    @After
    fun tearDown() {
        scope.cancel()
    }

    // The controller's interpolation ticker never finishes, and it shares this scheduler, so
    // runTest could never go idle. Cancel it when the body ends rather than in @After.
    private fun controllerTest(body: suspend TestScope.() -> Unit) = runTest(dispatcher) {
        try {
            body()
        } finally {
            scope.cancel()
        }
    }

    private fun remoteDevice() = ConnectedDevice(id = deviceId, name = "TV", deviceType = "android_tv")

    private fun remoteState(
        position: Double = 5.0,
        isPlaying: Boolean = true,
        receivedAt: Long = nowMs,
        volume: Float = 1f,
        shuffle: Boolean = false,
        repeat: String = "off",
    ) = RemotePlaybackState(
        currentTrack = RemoteTrackInfo("t1", "Track", "Artist", "Album", 200_000L, null),
        position = position,
        isPlaying = isPlaying,
        volume = volume,
        muted = false,
        shuffle = shuffle,
        repeat = repeat,
        timestamp = 0L,
        receivedAt = receivedAt,
        queuePosition = 0,
    )

    private fun enterRemote() {
        controller.initialize()
        devices.value = listOf(remoteDevice())
        listFresh.value = true
        modeManager.enterRemoteMode(deviceId, "TV")
        dispatcher.scheduler.runCurrent()
    }

    @Test
    fun `remote mode survives the device list being emptied by a disconnect`() = controllerTest {
        enterRemote()
        states.value = mapOf(deviceId to remoteState())
        runCurrent()

        // Our socket dropped: PlaybackSessionHandler clears everything and marks the list stale.
        listFresh.value = false
        devices.value = emptyList()
        states.value = emptyMap()
        runCurrent()

        assertThat(modeManager.mode.value).isEqualTo(PlaybackMode.Remote(deviceId, "TV"))
    }

    @Test
    fun `remote mode exits when a fresh device list no longer contains the target`() = controllerTest {
        enterRemote()

        devices.value = emptyList()
        listFresh.value = true
        runCurrent()

        assertThat(modeManager.mode.value).isEqualTo(PlaybackMode.Local)
    }

    @Test
    fun `remote mode exits after reconnect if the target is gone`() = controllerTest {
        enterRemote()
        listFresh.value = false
        devices.value = emptyList()
        runCurrent()
        assertThat(modeManager.mode.value).isEqualTo(PlaybackMode.Remote(deviceId, "TV"))

        // Welcome after reconnect: list is fresh and the TV is not in it.
        devices.value = listOf(ConnectedDevice(id = 1, name = "Me", deviceType = "android"))
        listFresh.value = true
        runCurrent()

        assertThat(modeManager.mode.value).isEqualTo(PlaybackMode.Local)
    }

    @Test
    fun `progress is interpolated from receivedAt and does not reset on device list changes`() = controllerTest {
        enterRemote()
        states.value = mapOf(deviceId to remoteState(position = 5.0, receivedAt = nowMs - 10_000L))
        runCurrent()

        advanceTimeBy(250)
        runCurrent()
        assertThat(controller.currentTrackProgressSec.value).isEqualTo(15)

        // Same state re-emitted because the device list changed. Must not snap back to 5 s.
        devices.value = listOf(remoteDevice(), ConnectedDevice(id = 9, name = "Web", deviceType = "web"))
        runCurrent()
        nowMs += 3_000L
        advanceTimeBy(250)
        runCurrent()

        assertThat(controller.currentTrackProgressSec.value).isEqualTo(18)
    }

    @Test
    fun `leaving remote mode resets volume shuffle and repeat`() = controllerTest {
        enterRemote()
        states.value = mapOf(deviceId to remoteState(volume = 0.3f, shuffle = true, repeat = "all"))
        runCurrent()
        assertThat(controller.volumeState.value.volume).isEqualTo(0.3f)
        assertThat(controller.shuffleEnabled.value).isTrue()
        assertThat(controller.repeatMode.value).isEqualTo(RepeatMode.ALL)

        modeManager.exitRemoteMode()
        runCurrent()

        assertThat(controller.isActive.value).isFalse()
        assertThat(controller.volumeState.value.volume).isEqualTo(1f)
        assertThat(controller.shuffleEnabled.value).isFalse()
        assertThat(controller.repeatMode.value).isEqualTo(RepeatMode.OFF)
    }
}
