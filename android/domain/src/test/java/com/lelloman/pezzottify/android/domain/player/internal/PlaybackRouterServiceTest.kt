package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.RadioCreationController
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import io.mockk.verifyOrder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

/** The media service hosts the remote session, so entering remote mode has to bring it up. */
@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackRouterServiceTest {

    private val dispatcher = StandardTestDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)
    private val loggerFactory = mockk<LoggerFactory> {
        val logger = mockk<Logger>(relaxed = true)
        every { getLogger(any<String>()) } returns logger
        every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns logger
        every { getValue(any(), any()) } returns logger
    }
    private val modeManager = PlaybackModeManager(loggerFactory)
    private val localPlayer = stubbedFlows(mockk<PlayerImpl>(relaxed = true))
    private val remoteController = mockk<RemotePlaybackController>(relaxed = true).also {
        every { it.isActive } returns MutableStateFlow(false)
        every { it.isPlaying } returns MutableStateFlow(false)
        every { it.volumeState } returns MutableStateFlow(com.lelloman.pezzottify.android.domain.player.VolumeState(1f, false))
        every { it.currentTrackIndex } returns MutableStateFlow(null)
        every { it.currentTrackPercent } returns MutableStateFlow(null)
        every { it.currentTrackProgressSec } returns MutableStateFlow(null)
        every { it.currentTrackDurationSeconds } returns MutableStateFlow(null)
        every { it.playerError } returns MutableStateFlow(null)
        every { it.shuffleEnabled } returns MutableStateFlow(false)
        every { it.repeatMode } returns MutableStateFlow(com.lelloman.pezzottify.android.domain.player.RepeatMode.OFF)
        every { it.seekEvents } returns kotlinx.coroutines.flow.MutableSharedFlow()
    }

    private fun stubbedFlows(player: PlayerImpl): PlayerImpl = player.also {
        every { it.isActive } returns MutableStateFlow(false)
        every { it.isPlaying } returns MutableStateFlow(false)
        every { it.volumeState } returns MutableStateFlow(com.lelloman.pezzottify.android.domain.player.VolumeState(1f, false))
        every { it.currentTrackIndex } returns MutableStateFlow(null)
        every { it.currentTrackPercent } returns MutableStateFlow(null)
        every { it.currentTrackProgressSec } returns MutableStateFlow(null)
        every { it.currentTrackDurationSeconds } returns MutableStateFlow(null)
        every { it.playerError } returns MutableStateFlow(null)
        every { it.shuffleEnabled } returns MutableStateFlow(false)
        every { it.repeatMode } returns MutableStateFlow(com.lelloman.pezzottify.android.domain.player.RepeatMode.OFF)
        every { it.seekEvents } returns kotlinx.coroutines.flow.MutableSharedFlow()
        every { it.playbackPlaylist } returns MutableStateFlow(null)
        every { it.canGoToPreviousPlaylist } returns MutableStateFlow(false)
        every { it.canGoToNextPlaylist } returns MutableStateFlow(false)
        every { it.radioContinuationError } returns MutableStateFlow(false)
    }

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
        PlaybackRouter(
            localPlayer = localPlayer,
            remoteController = remoteController,
            playbackModeManager = modeManager,
            playbackSessionHandler = mockk<PlaybackSessionHandler>(relaxed = true),
            radioCreation = mockk<RadioCreationController>(relaxed = true),
            scope = scope,
            loggerFactory = loggerFactory,
        ).initialize()
        dispatcher.scheduler.runCurrent()
    }

    @After
    fun tearDown() {
        scope.cancel()
        Dispatchers.resetMain()
    }

    @Test
    fun `local mode does not touch the media service`() {
        verify(exactly = 0) { localPlayer.ensureMediaServiceRunning() }
    }

    @Test
    fun `entering remote mode clears local playback and keeps the service running`() {
        modeManager.enterRemoteMode(7, "TV")
        dispatcher.scheduler.runCurrent()

        verifyOrder {
            localPlayer.clearSession()
            localPlayer.ensureMediaServiceRunning()
        }
    }
}
