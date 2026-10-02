package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.internal.PlayerImpl
import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationRequest
import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationResult
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class GravityPlaybackTest {
    private val dispatcher = StandardTestDispatcher()
    private val index = MutableStateFlow<Int?>(0)
    private val playing = MutableStateFlow(true)
    private val smartEnabled = MutableStateFlow(true)
    private val api = mockk<RemoteApiClient>()
    private val platform = mockk<PlatformPlayer>(relaxed = true)
    private val requests = mutableListOf<ContinuationRequest>()
    private var served = 0
    private var diagnostics: GravityDiagnostics? = null
    private fun ids(count: Int) = (0 until count).map { "track-$it" }

    @Before fun setup() {
        Dispatchers.setMain(dispatcher)
        every { platform.currentTrackIndex } returns index
        every { platform.currentTrackProgressSec } returns MutableStateFlow<Int?>(42)
        every { platform.isPlaying } returns playing
        every { platform.isActive } returns MutableStateFlow(true)
        every { platform.shuffleEnabled } returns MutableStateFlow(false)
        every { platform.repeatMode } returns MutableStateFlow(RepeatMode.OFF)
        // Each call serves a fresh suggestion; the player keeps asking until three tracks are queued ahead.
        coEvery { api.getContinuationRecommendations(capture(requests)) } coAnswers {
            served++
            RemoteApiResponse.Success(ContinuationResult(listOf("smart-$served"), diagnostics))
        }
    }
    @After fun teardown() { Dispatchers.resetMain() }

    private fun player(scope: CoroutineScope): PlayerImpl {
        val logger = mockk<Logger>(relaxed = true)
        val factory = mockk<LoggerFactory> { every { getValue(any(), any()) } returns logger }
        val config = mockk<ConfigStore> { every { baseUrl } returns MutableStateFlow("http://test") }
        val settings = mockk<UserSettingsStore> {
            every { isSmartContinuationEnabled } returns smartEnabled
            every { keepRadioOnQueueEdit } returns MutableStateFlow(true)
        }
        return PlayerImpl(mockk(), factory, platform, config, mockk(), mockk(relaxed = true), settings, api, scope).also { it.initialize() }
    }

    @Test fun `continuation anchors on user chosen tracks and excludes earlier suggestions`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadTrackIds(ids(3))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.gravity).isEqualTo(Gravity())
        diagnostics = GravityDiagnostics(recencyWeight = 0.2, at = 1)
        index.value = 2
        runCurrent()
        // First fetch asks for 3, gets 1, then immediately tops up to keep three tracks ahead.
        assertThat(requests).hasSize(2)
        with(requests[0]) {
            assertThat(sourceTrackIds).isEqualTo(ids(3))
            assertThat(recentTrackIds).isEqualTo(ids(3))
            assertThat(excludeTrackIds).isEqualTo(ids(3))
            assertThat(contextTrackIds).isEqualTo(ids(3))
            assertThat(count).isEqualTo(3)
            assertThat(destination).isNull()
        }
        with(requests[1]) {
            // The appended suggestion never joins the Source but is excluded from now on.
            assertThat(sourceTrackIds).isEqualTo(ids(3))
            assertThat(recentTrackIds).isEqualTo(ids(3))
            assertThat(excludeTrackIds).containsExactlyElementsIn(ids(3) + "smart-1")
            assertThat(count).isEqualTo(2)
        }
        val playlist = player.playbackPlaylist.value!!
        assertThat(playlist.tracksIds).isEqualTo(ids(3) + listOf("smart-1", "smart-2"))
        assertThat(playlist.gravity!!.autoTrackIds).containsExactly("smart-1", "smart-2").inOrder()
        assertThat(playlist.gravity!!.lastDiagnostics?.recencyWeight).isEqualTo(0.2)
        assertThat(playlist.context).isEqualTo(PlaybackPlaylistContext.UserMix)

        index.value = 4
        runCurrent()
        // Playing the last track fetches again (3, then a top-up of 2) until three tracks are ahead.
        assertThat(requests).hasSize(4)
        with(requests[2]) {
            assertThat(sourceTrackIds).isEqualTo(ids(3))
            assertThat(recentTrackIds).isEqualTo(ids(3) + listOf("smart-1", "smart-2"))
            assertThat(excludeTrackIds).containsExactlyElementsIn(ids(3) + listOf("smart-1", "smart-2"))
        }
        assertThat(player.playbackPlaylist.value!!.gravity!!.autoTrackIds)
            .containsExactly("smart-1", "smart-2", "smart-3", "smart-4").inOrder()
    }

    @Test fun `removed suggestions stay excluded and manual re-add promotes them`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadTrackIds(ids(3))
        runCurrent()
        index.value = 2
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.tracksIds).isEqualTo(ids(3) + listOf("smart-1", "smart-2"))
        player.removeTrackFromPlaylist("smart-1")
        runCurrent()
        // Removing the suggestion refills the queue, but the removed id stays excluded.
        assertThat(player.playbackPlaylist.value!!.tracksIds).isEqualTo(ids(3) + listOf("smart-2", "smart-3"))
        assertThat(requests.last().excludeTrackIds).contains("smart-1")
        assertThat(player.playbackPlaylist.value!!.gravity!!.autoTrackIds).containsExactly("smart-1", "smart-2", "smart-3").inOrder()

        player.addTracksToPlaylist(listOf("smart-1"))
        runCurrent()
        val gravity = player.playbackPlaylist.value!!.gravity!!
        assertThat(gravity.autoTrackIds).containsExactly("smart-2", "smart-3").inOrder()
        assertThat(player.playbackPlaylist.value!!.tracksIds).contains("smart-1")
        assertThat(requests.last().sourceTrackIds).doesNotContain("smart-2")
    }

    @Test fun `queue edits keep gravity and provenance`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadTrackIds(ids(2))
        runCurrent()
        index.value = 1
        runCurrent()
        val before = player.playbackPlaylist.value!!.gravity!!
        assertThat(before.autoTrackIds).containsExactly("smart-1", "smart-2").inOrder()
        player.moveTrack(0, 1)
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.gravity).isEqualTo(before)
        assertThat(player.playbackPlaylist.value!!.context).isEqualTo(PlaybackPlaylistContext.UserMix)
        player.removeTrackAtIndex(0)
        runCurrent()
        // The removal brings the queue back under the refill threshold, so one more suggestion lands.
        assertThat(player.playbackPlaylist.value!!.gravity!!.autoTrackIds)
            .containsExactly("smart-1", "smart-2", "smart-3").inOrder()
        assertThat(player.playbackPlaylist.value!!.gravity!!.source).isEqualTo(before.source)
    }

    @Test fun `radio playlists carry no gravity and ignore setGravity`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        val context = PlaybackPlaylistContext.Radio("greatest_hits", "artist", "artist", "Artist", 25)
        player.loadRadio(ids(10), context, RadioContinuation.create(ids(10), ids(25)))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.gravity).isNull()
        player.setGravity(Gravity().withDestination(GravityReference("artist", "a1"), 3))
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.gravity).isNull()
        index.value = 8
        runCurrent()
        coVerify(exactly = 0) { api.getContinuationRecommendations(any()) }
    }

    @Test fun `setGravity replaces gravity and destination steers the next request`() = runTest(dispatcher) {
        val player = player(backgroundScope)
        player.loadTrackIds(ids(3))
        runCurrent()
        val steered = Gravity().withDestination(GravityReference("artist", "a1", label = "A"), stepsTotal = 2)
        player.setGravity(steered)
        runCurrent()
        assertThat(player.playbackPlaylist.value!!.gravity).isEqualTo(steered)
        index.value = 2
        runCurrent()
        // Two suggestions were appended, so progress advanced 0 -> 0.5 and then arrived.
        assertThat(requests).hasSize(2)
        assertThat(requests[0].destination?.entityId).isEqualTo("a1")
        assertThat(requests[0].progress).isEqualTo(0.0)
        assertThat(requests[1].destination?.entityId).isEqualTo("a1")
        assertThat(requests[1].progress).isWithin(1e-9).of(0.5)
        val arrived = player.playbackPlaylist.value!!.gravity!!
        assertThat(arrived.destination).isNull()
        assertThat(arrived.stepsDone).isEqualTo(0)
        assertThat(arrived.source.kind).isEqualTo("references")
        assertThat(arrived.source.references.single().entityId).isEqualTo("a1")

        index.value = 4
        runCurrent()
        with(requests.last()) {
            assertThat(destination).isNull()
            assertThat(progress).isNull()
            assertThat(sourceTrackIds).isEmpty()
            assertThat(sourceReferences.single().entityId).isEqualTo("a1")
        }
    }
}
