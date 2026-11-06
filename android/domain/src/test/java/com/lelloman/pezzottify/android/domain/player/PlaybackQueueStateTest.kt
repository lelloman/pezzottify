package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class PlaybackQueueStateTest {

    private fun track(id: String) = TrackMetadata(
        trackId = id,
        trackName = id,
        artistNames = emptyList(),
        albumId = "",
        albumName = "",
        artworkUrl = null,
        durationSeconds = 1,
    )

    private fun queue(size: Int, index: Int) = PlaybackQueueState(List(size) { track("t$it") }, index)

    @Test
    fun `skip buttons follow the position in the queue`() {
        assertThat(queue(3, 0).hasPrevious).isFalse()
        assertThat(queue(3, 0).hasNext).isTrue()
        assertThat(queue(3, 1).hasPrevious).isTrue()
        assertThat(queue(3, 1).hasNext).isTrue()
        assertThat(queue(3, 2).hasPrevious).isTrue()
        assertThat(queue(3, 2).hasNext).isFalse()
    }

    @Test
    fun `a remote device reporting a position before its queue arrives cannot be skipped`() {
        // Only the current track is known yet, even though the remote is further along.
        val fallback = PlaybackQueueState(listOf(track("t")), currentIndex = 0)
        assertThat(fallback.hasNext).isFalse()
        assertThat(fallback.hasPrevious).isFalse()

        val empty = PlaybackQueueState(emptyList(), currentIndex = 4)
        assertThat(empty.hasNext).isFalse()
        assertThat(empty.hasPrevious).isFalse()
    }
}
