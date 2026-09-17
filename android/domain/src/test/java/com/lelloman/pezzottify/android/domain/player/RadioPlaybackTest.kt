package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.*
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RadioPlaybackTest {
    private val dispatcher = StandardTestDispatcher()
    private val index = MutableStateFlow<Int?>(0)
    private val playing = MutableStateFlow(true)
    private val keepGoing = MutableStateFlow(true)
    private val smartEnabled = MutableStateFlow(true)
    private val api = mockk<RemoteApiClient>()
    private val platform = mockk<PlatformPlayer>(relaxed = true)
    private val context = PlaybackPlaylistContext.Radio("greatest_hits", "artist", "artist", "Artist", 25)
    private fun ids(count: Int) = (0 until count).map { "track-$it" }

    @Before fun setup() {
        Dispatchers.setMain(dispatcher)
        every { platform.currentTrackIndex } returns index
        every { platform.currentTrackProgressSec } returns MutableStateFlow<Int?>(42)
        every { platform.isPlaying } returns playing
        every { platform.isActive } returns MutableStateFlow(true)
        every { platform.shuffleEnabled } returns MutableStateFlow(false)
        every { platform.repeatMode } returns MutableStateFlow(RepeatMode.OFF)
        every { platform.setIsPlaying(any()) } answers { playing.value = firstArg() }
        every { platform.removeMediaItem(0) } answers { index.value = index.value!! - 1 }
    }
    @After fun teardown() { Dispatchers.resetMain() }

    private fun player(scope: CoroutineScope): PlayerImpl {
        val logger = mockk<Logger>(relaxed = true)
        val factory = mockk<LoggerFactory> { every { getValue(any(), any()) } returns logger }
        val config = mockk<ConfigStore> { every { baseUrl } returns MutableStateFlow("http://test") }
        val settings = mockk<UserSettingsStore> {
            every { isSmartContinuationEnabled } returns smartEnabled
            every { keepRadioOnQueueEdit } returns keepGoing
        }
        return PlayerImpl(mockk(), factory, platform, config, mockk(), mockk(relaxed = true), settings, api, scope).also { it.initialize() }
    }

    @Test fun `snapshot extends at track nine and exhausts without smart continuation`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context, RadioContinuation.create(ids(10), ids(25)))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).hasSize(10)
        index.value = 8
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).hasSize(20)
        index.value = 18
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).containsExactlyElementsIn(ids(25)).inOrder()
        assertThat(player.playbackPlaylist.value!!.continuation!!.status).isEqualTo("exhausted")
        index.value = 24
        runCurrent()
        coVerify(exactly = 0) { api.getContinuationRecommendations(any(), any(), any()) }
        coVerify(exactly = 0) { api.continueRadio(any(), any(), any()) }
    }

    @Test fun `manual edits either keep artist generation or stop all continuation`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context, RadioContinuation.create(ids(10), ids(30)))
        runCurrent()
        player.removeTrackAtIndex(3)
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.continuation!!.status).isEqualTo("active")
        keepGoing.value = false
        player.moveTrack(1, 2)
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.continuation!!.status).isEqualTo("stopped")
        index.value = 8
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).hasSize(9)
        coVerify(exactly = 0) { api.getContinuationRecommendations(any(), any(), any()) }
    }

    @Test fun `large snapshot trims played history and keeps current audio`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context, RadioContinuation.create(ids(10), ids(530)))
        runCurrent()
        while (player.playbackPlaylist.value!!.continuation!!.status == "active") {
            index.value = player.playbackPlaylist.value!!.tracksIds.size - 2
            val current = player.playbackPlaylist.value!!.tracksIds[index.value!!]
            runCurrent()
            assertThat(player.playbackPlaylist.value!!.tracksIds.size).isAtMost(500)
            assertThat(player.playbackPlaylist.value!!.tracksIds[index.value!!]).isEqualTo(current)
        }
        assertThat(player.playbackPlaylist.value!!.continuation!!.seenTrackIds).containsExactlyElementsIn(ids(530)).inOrder()
        verify(exactly = 1) { platform.loadPlaylist(any(), any()) }
        verify(exactly = 30) { platform.removeMediaItem(0) }
    }

    @Test fun `response from a replaced radio cannot append to a new session`() = runTest(dispatcher) {
        val response = CompletableDeferred<RemoteApiResponse<List<String>>>()
        coEvery { api.continueRadio(any(), any(), any()) } coAnswers { response.await() }
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context.copy(source = "basic"))
        runCurrent()
        index.value = 8
        runCurrent()
        coVerify(exactly = 1) { api.continueRadio(any(), any(), any()) }
        player.loadRadio(listOf("replacement"), context, RadioContinuation.create(listOf("replacement"), listOf("replacement")))
        runCurrent()
        response.complete(RemoteApiResponse.Success(listOf("stale")))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).containsExactly("replacement")
    }

    @Test fun `removing the final loaded track can refill without forgetting exclusions`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        // Keep the initial queue away from its refill threshold until all removals are queued.
        player.loadRadio(ids(10), context, RadioContinuation.create(ids(10), ids(30)))
        runCurrent()
        for (i in 9 downTo 0) player.removeTrackAtIndex(i)
        // A platform with an empty queue has no current index.
        every { platform.removeMediaItem(0) } answers { index.value = null; playing.value = false }
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).containsExactlyElementsIn(ids(20).drop(10)).inOrder()
        verify { platform.loadTrack(0, 0) }
    }

    @Test fun `network failures retry finitely and explicit retry does not restart paused audio`() = runTest(dispatcher) {
        smartEnabled.value = false
        coEvery { api.continueRadio(any(), any(), any()) } returns RemoteApiResponse.Error.Network
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context.copy(source = "basic"))
        runCurrent()
        index.value = 8
        runCurrent()
        advanceTimeBy(4501)
        runCurrent()
        coVerify(exactly = 3) { api.continueRadio(any(), any(), any()) }
        assertThat(player.radioContinuationError.value).isTrue()
        assertThat(player.playbackPlaylist.value!!.continuation!!.status).isEqualTo("active")
        playing.value = false
        runCurrent()
        coEvery { api.continueRadio(any(), any(), any()) } returns RemoteApiResponse.Success(listOf("new-1", "new-2"))
        player.retryRadioContinuation()
        runCurrent()
        assertThat(player.radioContinuationError.value).isFalse()
        assertThat(player.playbackPlaylist.value!!.tracksIds).contains("new-1")
        verify(exactly = 0) { platform.setIsPlaying(true) }
        coVerify(exactly = 0) { api.getContinuationRecommendations(any(), any(), any()) }
    }

    @Test fun `pause cancels an outstanding batch without restarting playback`() = runTest(dispatcher) {
        val response = CompletableDeferred<RemoteApiResponse<List<String>>>()
        coEvery { api.continueRadio(any(), any(), any()) } coAnswers { response.await() }
        val player = player(backgroundScope)
        player.loadRadio(ids(10), context.copy(source = "basic"))
        runCurrent()
        index.value = 8
        runCurrent()
        player.setIsPlaying(false)
        runCurrent()
        response.complete(RemoteApiResponse.Success(listOf("late")))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).doesNotContain("late")
        verify(exactly = 0) { platform.setIsPlaying(true) }
    }

    @Test fun `Android wire payload includes default fields required by web`() {
        val payload = RadioContinuation.create(ids(10)).toWireJson()
        assertThat(payload["status"].toString()).isEqualTo("\"active\"")
        assertThat(payload["strategy"].toString()).isEqualTo("\"seeded_radio\"")
        assertThat(payload["next_index"].toString()).isEqualTo("0")
        assertThat(payload["ordered_track_ids"].toString()).isEqualTo("[]")
    }

    @Test fun `web continuation payload restores the same snapshot and stopped status`() {
        val json = """{"session_id":"web-session","strategy":"ranked_snapshot","status":"stopped","seen_track_ids":["one"],"ordered_track_ids":["one","two"],"next_index":1}"""
        val state = Json.decodeFromString(RadioContinuation.serializer(), json)
        assertThat(state.nextBatch().first).containsExactly("two")
        assertThat(state.status).isEqualTo("stopped")
        assertThat(Json.decodeFromString(RadioContinuation.serializer(), Json.encodeToString(RadioContinuation.serializer(), state))).isEqualTo(state)
    }
}
