package com.lelloman.pezzottify.android.localdata.internal.player

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.Gravity
import com.lelloman.pezzottify.android.domain.player.GravityKnobs
import com.lelloman.pezzottify.android.domain.player.GravityReference
import com.lelloman.pezzottify.android.domain.player.RadioContinuation
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class RadioPlaybackPersistenceTest {
    @Test fun `work versions context survives restoration and editing`() = runTest {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val tracks = listOf("one", "two")
        val playlist = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Radio("work_versions", "track", "one", "Composition", 2,
                settings = kotlinx.serialization.json.Json.parseToJsonElement("""{"work_id":"work"}""") as kotlinx.serialization.json.JsonObject, isEdited = true),
            tracksIds = tracks,
            continuation = RadioContinuation.create(tracks, tracks).edited(tracks, false),
        )
        PlaybackStateStoreImpl(context, dispatcher).saveState(playlist, 1, 0, false)
        assertThat(PlaybackStateStoreImpl(context, dispatcher).loadState()!!.playlist).isEqualTo(playlist)
    }

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

    @Test fun `gravity with destination auto ids and knobs survives restoration`() = runTest {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val playlist = PlaybackPlaylist(
            context = PlaybackPlaylistContext.UserMix,
            tracksIds = listOf("one", "two", "s1"),
            gravity = Gravity()
                .withDestination(GravityReference("artist", "a1", label = "A"), stepsTotal = 9)
                .appendedAuto(listOf("s1"))
                .withKnobs(GravityKnobs(recencyWeight = 0.4, mode = "similar")),
        )
        PlaybackStateStoreImpl(context, dispatcher).saveState(playlist, 2, 10, true)
        val restored = PlaybackStateStoreImpl(context, dispatcher).loadState()!!.playlist
        assertThat(restored).isEqualTo(playlist)
        assertThat(restored.gravity!!.stepsDone).isEqualTo(1)
    }

    @Test fun `legacy saved state without gravity still decodes`() = runTest {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val legacy = """{"context":{"type":"com.lelloman.pezzottify.android.localdata.internal.player.PersistableContext.UserMix"},"tracksIds":["one","two"],"currentTrackIndex":1,"positionMs":5,"isPlaying":false,"savedAtMs":${System.currentTimeMillis()}}"""
        context.getSharedPreferences("PlaybackStateStore", Context.MODE_PRIVATE).edit()
            .putString("saved_state", legacy).commit()
        val restored = PlaybackStateStoreImpl(context, dispatcher).loadState()
        assertThat(restored).isNotNull()
        assertThat(restored!!.playlist.tracksIds).containsExactly("one", "two").inOrder()
        assertThat(restored.playlist.gravity).isNull()
    }
}
