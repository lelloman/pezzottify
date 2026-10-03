package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Test

class PlaybackGravityTest {

    private val mode = MutableStateFlow<PlaybackMode>(PlaybackMode.Local)
    private val playlist = MutableStateFlow<PlaybackPlaylist?>(null)
    private val remoteQueues = MutableStateFlow<Map<Int, List<String>>>(emptyMap())
    private val remoteContexts = MutableStateFlow<Map<Int, PlaybackPlaylistContext?>>(emptyMap())
    private val remoteGravities = MutableStateFlow<Map<Int, Gravity?>>(emptyMap())

    private val player = mockk<PezzottifyPlayer>(relaxed = true) {
        every { playbackPlaylist } returns playlist
    }
    private val modeManager = mockk<PlaybackModeManager> { every { mode } returns this@PlaybackGravityTest.mode }
    private val session = mockk<PlaybackSessionHandler> {
        every { otherDeviceQueues } returns remoteQueues
        every { otherDeviceQueueContexts } returns remoteContexts
        every { otherDeviceGravities } returns remoteGravities
    }
    private val playbackGravity = PlaybackGravity(player, modeManager, session)

    private val artist = GravityReference("artist", "a1", "Artist")

    @Test
    fun `local mix is steerable and missing gravity defaults`() {
        playlist.value = PlaybackPlaylist(PlaybackPlaylistContext.UserMix, listOf("t1"))
        val state = playbackGravity.current()
        assertThat(state.isSteerable).isTrue()
        assertThat(state.gravity).isEqualTo(Gravity())
        assertThat(state.isRemote).isFalse()
    }

    @Test
    fun `radio and empty queues are not steerable`() {
        assertThat(playbackGravity.current().isSteerable).isFalse()
        assertThat(playbackGravity.update { it.withDestination(artist) }).isFalse()

        playlist.value = PlaybackPlaylist(
            PlaybackPlaylistContext.Radio("basic", "track", "t1", "T", 50),
            listOf("t1"),
        )
        assertThat(playbackGravity.current().isRadio).isTrue()
        assertThat(playbackGravity.current().gravity).isNull()
        assertThat(playbackGravity.update { it.withDestination(artist) }).isFalse()
        verify(exactly = 0) { player.setGravity(any()) }
    }

    @Test
    fun `remote mode reads the controlled device gravity`() {
        mode.value = PlaybackMode.Remote(7, "Desk")
        remoteQueues.value = mapOf(7 to listOf("t1", "s1"))
        remoteGravities.value = mapOf(7 to Gravity(autoTrackIds = listOf("s1")))

        val state = playbackGravity.current()
        assertThat(state.isRemote).isTrue()
        assertThat(state.trackIds).containsExactly("t1", "s1").inOrder()
        assertThat(state.gravity?.autoTrackIds).containsExactly("s1")

        val sent = slot<Gravity>()
        every { player.setGravity(capture(sent)) } returns Unit
        assertThat(playbackGravity.update { it.withStepsTotal(5) }).isTrue()
        assertThat(sent.captured.stepsTotal).isEqualTo(5)
        assertThat(sent.captured.autoTrackIds).containsExactly("s1")
    }

    @Test
    fun `steering toward a destination drops explore mode`() {
        val gravity = Gravity(knobs = GravityKnobs(mode = MODE_EXPLORE), stepsDone = 3)
            .steeringToward(artist, 12)
        assertThat(gravity.destination).containsExactly(artist)
        assertThat(gravity.stepsTotal).isEqualTo(12)
        assertThat(gravity.stepsDone).isEqualTo(0)
        assertThat(gravity.knobs.mode).isNull()
    }

    @Test
    fun `set playback destination resolves the label from statics`() = runTest {
        playlist.value = PlaybackPlaylist(PlaybackPlaylistContext.UserMix, listOf("t1"))
        val statics = mockk<StaticsStore> {
            every { getArtist("a1") } returns flowOf(mockk { every { name } returns "Resolved" })
        }
        val sent = slot<Gravity>()
        every { player.setGravity(capture(sent)) } returns Unit

        val applied = SetPlaybackDestination(playbackGravity, statics)("artist", "a1", 8)

        assertThat(applied).isTrue()
        assertThat(sent.captured.destination).containsExactly(GravityReference("artist", "a1", "Resolved"))
        assertThat(sent.captured.stepsTotal).isEqualTo(8)
    }

    @Test
    fun `adding to the destination grows the mix, keeps progress and drops explore`() = runTest {
        val jazz = GravityReference("concept", "audioset:Jazz", "Jazz")
        playlist.value = PlaybackPlaylist(
            PlaybackPlaylistContext.UserMix,
            listOf("t1"),
            gravity = Gravity(knobs = GravityKnobs(mode = MODE_EXPLORE)).steeringToward(artist, 10).copy(stepsDone = 4),
        )
        val sent = slot<Gravity>()
        every { player.setGravity(capture(sent)) } returns Unit
        val useCase = SetPlaybackDestination(playbackGravity, mockk(relaxed = true))

        assertThat(useCase.hasDestination()).isTrue()
        assertThat(useCase.add("concept", "audioset:Jazz", "Jazz")).isTrue()

        assertThat(sent.captured.destination).containsExactly(artist, jazz).inOrder()
        assertThat(sent.captured.stepsDone).isEqualTo(4)
        assertThat(sent.captured.knobs.mode).isNull()
    }

    @Test
    fun `adding without a destination starts a single component mix`() = runTest {
        playlist.value = PlaybackPlaylist(PlaybackPlaylistContext.UserMix, listOf("t1"))
        val sent = slot<Gravity>()
        every { player.setGravity(capture(sent)) } returns Unit
        val useCase = SetPlaybackDestination(playbackGravity, mockk(relaxed = true))

        assertThat(useCase.hasDestination()).isFalse()
        assertThat(useCase.add("concept", "recorded:1960s", "Recorded in the 1960s")).isTrue()
        assertThat(sent.captured.destination!!.single().entityId).isEqualTo("recorded:1960s")
    }

    @Test
    fun `set playback destination refuses a radio queue`() = runTest {
        playlist.value = PlaybackPlaylist(
            PlaybackPlaylistContext.Radio("basic", "track", "t1", "T", 50),
            listOf("t1"),
        )
        val applied = SetPlaybackDestination(playbackGravity, mockk(relaxed = true))("artist", "a1", 8, "A")
        assertThat(applied).isFalse()
        verify(exactly = 0) { player.setGravity(any()) }
    }
}
