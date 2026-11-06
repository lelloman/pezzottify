package com.lelloman.pezzottify.android.player

import android.os.Looper
import androidx.media3.common.Player
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class RemoteSessionPlayerTest {

    private val controls = mockk<ControlsAndStatePlayer>(relaxed = true)
    private var nowMs = 1_000_000L
    private val player = RemoteSessionPlayer(Looper.getMainLooper(), controls, "TV") { nowMs }

    private fun track(id: String, durationMs: Long = 200_000L) =
        RemoteSessionTrack(id, "Title $id", "Artist $id", "Album", durationMs)

    private fun snapshot(
        tracks: List<RemoteSessionTrack> = listOf(track("a"), track("b"), track("c")),
        index: Int = 1,
        isPlaying: Boolean = true,
        positionMs: Long = 10_000L,
    ) = RemoteSessionSnapshot("TV", tracks, index, isPlaying, positionMs)

    private fun idle() = shadowOf(Looper.getMainLooper()).idle()

    @Test
    fun `before any state arrives there is a placeholder so the service can stay in the foreground`() {
        assertEquals(1, player.mediaItemCount)
        assertEquals("TV", player.mediaMetadata.title.toString())
        assertEquals(Player.STATE_READY, player.playbackState)
        assertFalse(player.playWhenReady)
        assertFalse(player.isCurrentMediaItemSeekable)
    }

    @Test
    fun `mirrors the remote queue and playback state`() {
        player.update(snapshot())
        idle()

        assertEquals(3, player.mediaItemCount)
        assertEquals(1, player.currentMediaItemIndex)
        assertTrue(player.playWhenReady)
        assertEquals("Title b", player.mediaMetadata.title.toString())
        assertEquals("Artist b · TV", player.mediaMetadata.artist.toString())
        assertEquals(200_000L, player.duration)
    }

    @Test
    fun `a track queued twice does not break the playlist`() {
        player.update(snapshot(tracks = listOf(track("a"), track("a"))))
        idle()

        assertEquals(2, player.mediaItemCount)
    }

    @Test
    fun `artwork applies only to the track it belongs to`() {
        player.update(snapshot())
        player.setArtwork("b", byteArrayOf(1, 2, 3))
        idle()
        assertEquals(3, player.mediaMetadata.artworkData?.size)

        player.setArtwork("c", byteArrayOf(1))
        idle()
        assertEquals(null, player.mediaMetadata.artworkData)
    }

    @Test
    fun `steady progress does not republish state but a jump does`() {
        var events = 0
        player.addListener(object : Player.Listener {
            override fun onPositionDiscontinuity(
                oldPosition: Player.PositionInfo,
                newPosition: Player.PositionInfo,
                reason: Int,
            ) {
                events++
            }
        })
        player.update(snapshot(positionMs = 10_000L))
        idle()
        events = 0

        nowMs += 5_000L
        player.update(snapshot(positionMs = 15_000L))
        idle()
        assertEquals(0, events)

        player.update(snapshot(positionMs = 90_000L))
        idle()
        assertEquals(1, events)
        assertEquals(90_000L, player.currentPosition)
    }

    @Test
    fun `transport controls become commands to the remote device`() {
        player.update(snapshot(isPlaying = false))
        idle()

        player.play()
        verify { controls.setIsPlaying(true) }

        player.update(snapshot(isPlaying = true))
        idle()
        player.pause()
        verify { controls.setIsPlaying(false) }

        player.seekToNextMediaItem()
        verify { controls.skipToNextTrack() }

        player.seekToPreviousMediaItem()
        verify { controls.skipToPreviousTrack() }

        player.seekTo(2, 0L)
        verify { controls.loadTrackIndex(2) }

        player.seekTo(50_000L)
        verify { controls.seekToPercentage(25f) }
    }

    @Test
    fun `releasing the session leaves the remote device playing`() {
        player.update(snapshot(isPlaying = true))
        idle()

        player.release()
        idle()

        verify(exactly = 0) { controls.setIsPlaying(any()) }
        verify(exactly = 0) { controls.stop() }
    }

    @Test
    fun `snapshots combine the queue with the routed player state`() = runBlocking {
        val metadata = TrackMetadata(
            trackId = "t",
            trackName = "Song",
            artistNames = listOf("One", "Two"),
            albumId = "al",
            albumName = "Album",
            artworkUrl = null,
            durationSeconds = 120,
        )
        every { controls.isPlaying } returns MutableStateFlow(true)
        every { controls.currentTrackProgressSec } returns MutableStateFlow(30)

        val result = remoteSessionSnapshots(
            "TV",
            controls,
            MutableStateFlow(PlaybackQueueState(listOf(metadata), currentIndex = 0)),
        ).first()

        assertEquals(
            RemoteSessionSnapshot(
                deviceName = "TV",
                tracks = listOf(RemoteSessionTrack("t", "Song", "One, Two", "Album", 120_000L)),
                currentIndex = 0,
                isPlaying = true,
                positionMs = 30_000L,
            ),
            result,
        )
    }
}
