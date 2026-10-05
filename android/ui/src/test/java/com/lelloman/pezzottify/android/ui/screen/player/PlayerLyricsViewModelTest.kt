package com.lelloman.pezzottify.android.ui.screen.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.TrackLyrics
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.usecase.GetTrackLyrics
import io.mockk.coEvery
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.*
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PlayerLyricsViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val api = mockk<RemoteApiClient>()
    @Before fun setup() { Dispatchers.setMain(dispatcher) }
    @After fun teardown() { Dispatchers.resetMain() }

    @Test fun `late previous track response is discarded and old lyrics clear immediately`() = runTest(dispatcher) {
        coEvery { api.getTrackLyrics("old") } coAnswers {
            withContext(NonCancellable) { delay(1000) }
            RemoteApiResponse.Success(TrackLyrics("old", "found", plainLyrics = "Old lyrics"))
        }
        coEvery { api.getTrackLyrics("new") } returns RemoteApiResponse.Success(TrackLyrics("new", "found", syncedLyrics = "[00:01.50]New line"))
        val vm = PlayerLyricsViewModel(GetTrackLyrics(api))
        vm.load("old")
        runCurrent()
        vm.load("new")
        assertThat(vm.state.value.hasLyrics).isFalse()
        advanceUntilIdle()
        assertThat(vm.state.value.trackId).isEqualTo("new")
        assertThat(vm.state.value.lines).containsExactly(LyricLine(1500, "New line"))
        vm.load("")
        assertThat(vm.state.value.hasLyrics).isFalse()
    }

    @Test fun `plain lyrics display but missing instrumental errors and mismatched tracks do not`() = runTest(dispatcher) {
        val vm = PlayerLyricsViewModel(GetTrackLyrics(api))
        coEvery { api.getTrackLyrics("t") } returns RemoteApiResponse.Success(TrackLyrics("t", "found", plainLyrics = "Plain\ntext"))
        vm.load("t"); advanceUntilIdle()
        assertThat(vm.state.value.plainText).isEqualTo("Plain\ntext")
        for (response in listOf(RemoteApiResponse.Success(null), RemoteApiResponse.Error.Network,
            RemoteApiResponse.Success(TrackLyrics("t", "instrumental")),
            RemoteApiResponse.Success(TrackLyrics("t", "not_found")),
            RemoteApiResponse.Success(TrackLyrics("other", "found", plainLyrics = "Wrong")))) {
            coEvery { api.getTrackLyrics("t") } returns response
            vm.load("t"); advanceUntilIdle()
            assertThat(vm.state.value.hasLyrics).isFalse()
        }
    }

    @Test fun `LRC supports fractions repeats offsets blank lines and backward seeks`() {
        val lines = parseSyncedLyrics("[ar:Example]\n[offset:100]\n[00:02.50][00:12.500]Repeat\n[00:01.1]First\n[00:03.00]\n[00:99]Bad")
        assertThat(lines).containsExactly(LyricLine(1000,"First"),LyricLine(2400,"Repeat"),LyricLine(2900,""),LyricLine(12400,"Repeat")).inOrder()
        assertThat(activeLyricIndex(lines, 0)).isEqualTo(-1)
        assertThat(activeLyricIndex(lines, 2400)).isEqualTo(1)
        assertThat(activeLyricIndex(lines, 15000)).isEqualTo(3)
        assertThat(activeLyricIndex(lines, 1000)).isEqualTo(0)
        assertThat(parseSyncedLyrics("[ar:Name]\nPlain text")).isEmpty()
    }
}
