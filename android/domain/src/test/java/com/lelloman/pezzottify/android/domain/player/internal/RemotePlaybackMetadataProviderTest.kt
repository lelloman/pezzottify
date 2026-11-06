package com.lelloman.pezzottify.android.domain.player.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState
import com.lelloman.pezzottify.android.domain.playbacksession.RemoteTrackInfo
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability
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
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RemotePlaybackMetadataProviderTest {

    private val dispatcher = StandardTestDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private val deviceId = 7
    private val states = MutableStateFlow<Map<Int, RemotePlaybackState>>(emptyMap())
    private val queues = MutableStateFlow<Map<Int, List<String>>>(emptyMap())
    private val contexts = MutableStateFlow<Map<Int, PlaybackPlaylistContext?>>(emptyMap())
    private val sessionHandler = mockk<PlaybackSessionHandler> {
        every { otherDeviceStates } returns states
        every { otherDeviceQueues } returns queues
        every { otherDeviceQueueContexts } returns contexts
    }
    private val modeManager = mockk<PlaybackModeManager> {
        every { mode } returns MutableStateFlow(PlaybackMode.Remote(deviceId, "TV"))
    }
    private val configStore = mockk<ConfigStore> {
        every { baseUrl } returns MutableStateFlow("http://server")
    }
    private val trackFlows = mutableMapOf<String, MutableStateFlow<StaticsItem<out Track>>>()
    private val staticsProvider = mockk<StaticsProvider> {
        every { provideTrack(any()) } answers {
            val id = firstArg<String>()
            trackFlows.getOrPut(id) { MutableStateFlow(StaticsItem.Loading(id)) }
        }
        every { provideAlbum(any()) } answers { MutableStateFlow(StaticsItem.Loading(firstArg())) }
        every { provideArtist(any()) } answers { MutableStateFlow(StaticsItem.Loading(firstArg())) }
    }
    private val loggerFactory = mockk<LoggerFactory> {
        val logger = mockk<Logger>(relaxed = true)
        every { getLogger(any<String>()) } returns logger
        every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns logger
        every { getValue(any(), any()) } returns logger
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    private fun remoteState(queuePosition: Int) = RemotePlaybackState(
        currentTrack = RemoteTrackInfo("t1", "Remote t1", "Remote artist", "Remote album", 200_000, "img"),
        position = 0.0,
        isPlaying = true,
        volume = 1f,
        muted = false,
        shuffle = false,
        repeat = "off",
        timestamp = 0L,
        queuePosition = queuePosition,
    )

    private fun track(id: String) = mockk<Track> {
        every { this@mockk.id } returns id
        every { name } returns "Track $id"
        every { albumId } returns "album-$id"
        every { artistsIds } returns listOf("artist-$id")
        every { durationSeconds } returns 180
        every { availability } returns TrackAvailability.Available
    }

    @Test
    fun `queue tracks loaded after the first pass patch the remote queue state`() = runTest(dispatcher) {
        val provider = RemotePlaybackMetadataProvider(
            sessionHandler, modeManager, configStore, staticsProvider, scope, loggerFactory,
        )
        states.value = mapOf(deviceId to remoteState(queuePosition = 0))
        queues.value = mapOf(deviceId to listOf("t1", "t2"))
        runCurrent()
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)

        advanceTimeBy(QueueMetadataResolver.DEFAULT_FIRST_PASS_TIMEOUT.inWholeMilliseconds + 1)
        runCurrent()
        var state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        // Unresolved current track falls back to the remote device's own info.
        assertThat(state.tracks.map { it.trackName }).containsExactly("Remote t1", "t2").inOrder()

        // The remote moves on, then t2's metadata lands: patch keeps the latest position.
        states.value = mapOf(deviceId to remoteState(queuePosition = 1))
        runCurrent()
        trackFlows.getValue("t2").value = StaticsItem.Loaded("t2", track("t2"))
        runCurrent()
        state = provider.queueState.value!!
        assertThat(state.tracks[1].trackName).isEqualTo("Track t2")
        assertThat(state.tracks[1].durationSeconds).isEqualTo(180)
        assertThat(state.currentIndex).isEqualTo(1)
    }
}
