package com.lelloman.pezzottify.android.localdata.internal.player

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.RadioContinuation
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class RadioPlaybackPersistenceTest {
    @Test fun `restoration preserves snapshot position exclusions and stopped status`() = runTest {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val tracks = (0 until 20).map { "track-$it" }
        val playlist = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Radio("greatest_hits", "artist", "artist", "Artist", 20, isEdited = true),
            tracksIds = tracks.take(10).drop(2),
            continuation = RadioContinuation.create(tracks.take(10), tracks).edited(tracks.take(10), false),
        )
        PlaybackStateStoreImpl(context, dispatcher).saveState(playlist, 7, 12345, false)
        val restored = PlaybackStateStoreImpl(context, dispatcher).loadState()!!
        assertThat(restored.playlist).isEqualTo(playlist)
        assertThat(restored.positionMs).isEqualTo(12345)
        assertThat(restored.currentTrackIndex).isEqualTo(7)
        assertThat(restored.isPlaying).isFalse()
    }
}
