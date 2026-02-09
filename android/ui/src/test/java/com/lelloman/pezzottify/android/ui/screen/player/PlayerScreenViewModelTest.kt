package com.lelloman.pezzottify.android.ui.screen.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PlayerScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: PlayerScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel() {
        viewModel = PlayerScreenViewModel(
            interactor = fakeInteractor,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state updates when playback state changes to loaded`() = runTest {
        val playbackState = PlayerScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Test Track",
            albumId = "album-1",
            albumName = "Test Album",
            albumImageUrl = "http://img.com/album1.jpg",
            artists = listOf(ArtistInfo("artist-1", "Test Artist")),
            trackPercent = 0.5f,
            trackProgressSec = 120,
            trackDurationSec = 240,
            hasNextTrack = true,
            hasPreviousTrack = false,
            volume = 0.8f,
            isMuted = false,
            shuffleEnabled = true,
            repeatMode = RepeatModeUi.ALL,
            playerError = null,
        )

        createViewModel()
        advanceUntilIdle()

        // Emit playback state
        fakeInteractor.playbackStateFlow.value = playbackState
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isPlaying).isTrue()
        assertThat(viewModel.state.value.trackProgressPercent).isEqualTo(0.5f)
        assertThat(viewModel.state.value.trackProgressSec).isEqualTo(120)
        assertThat(viewModel.state.value.hasNextTrack).isTrue()
        assertThat(viewModel.state.value.hasPreviousTrack).isFalse()
        assertThat(viewModel.state.value.volume).isEqualTo(0.8f)
        assertThat(viewModel.state.value.isMuted).isFalse()
        assertThat(viewModel.state.value.shuffleEnabled).isTrue()
        assertThat(viewModel.state.value.repeatMode).isEqualTo(RepeatModeUi.ALL)
    }

    @Test
    fun `state displays track info from playback state`() = runTest {
        val playbackState = createPlaybackState(
            trackId = "track-1",
            trackName = "Amazing Song",
            albumId = "album-1",
            artists = listOf(
                ArtistInfo("artist-1", "Singer One"),
                ArtistInfo("artist-2", "Singer Two"),
            ),
            trackDurationSec = 300,
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = playbackState
        advanceUntilIdle()

        assertThat(viewModel.state.value.trackId).isEqualTo("track-1")
        assertThat(viewModel.state.value.trackName).isEqualTo("Amazing Song")
        assertThat(viewModel.state.value.albumId).isEqualTo("album-1")
        assertThat(viewModel.state.value.artists).hasSize(2)
        assertThat(viewModel.state.value.artists[0].name).isEqualTo("Singer One")
        assertThat(viewModel.state.value.trackDurationSec).isEqualTo(300)
    }

    @Test
    fun `state displays album info from playback state`() = runTest {
        val playbackState = createPlaybackState(
            trackId = "track-1",
            albumId = "album-1",
            albumName = "Epic Album",
            albumImageUrl = "http://cdn.com/cover.jpg",
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = playbackState
        advanceUntilIdle()

        assertThat(viewModel.state.value.albumName).isEqualTo("Epic Album")
        assertThat(viewModel.state.value.albumImageUrl).isEqualTo("http://cdn.com/cover.jpg")
    }

    @Test
    fun `clickOnPlayPause calls interactor togglePlayPause`() = runTest {
        createViewModel()

        viewModel.clickOnPlayPause()

        assertThat(fakeInteractor.togglePlayPauseCalled).isTrue()
    }

    @Test
    fun `clickOnSkipNext calls interactor skipToNext`() = runTest {
        createViewModel()

        viewModel.clickOnSkipNext()

        assertThat(fakeInteractor.skipToNextCalled).isTrue()
    }

    @Test
    fun `clickOnSkipPrevious calls interactor skipToPrevious`() = runTest {
        createViewModel()

        viewModel.clickOnSkipPrevious()

        assertThat(fakeInteractor.skipToPreviousCalled).isTrue()
    }

    @Test
    fun `seekToPercent calls interactor with correct value`() = runTest {
        createViewModel()

        viewModel.seekToPercent(0.75f)

        assertThat(fakeInteractor.lastSeekPercent).isEqualTo(0.75f)
    }

    @Test
    fun `setVolume calls interactor with correct value`() = runTest {
        createViewModel()

        viewModel.setVolume(0.6f)

        assertThat(fakeInteractor.lastVolume).isEqualTo(0.6f)
    }

    @Test
    fun `toggleMute calls interactor toggleMute`() = runTest {
        createViewModel()

        viewModel.toggleMute()

        assertThat(fakeInteractor.toggleMuteCalled).isTrue()
    }

    @Test
    fun `clickOnShuffle calls interactor toggleShuffle`() = runTest {
        createViewModel()

        viewModel.clickOnShuffle()

        assertThat(fakeInteractor.toggleShuffleCalled).isTrue()
    }

    @Test
    fun `clickOnRepeat calls interactor cycleRepeatMode`() = runTest {
        createViewModel()

        viewModel.clickOnRepeat()

        assertThat(fakeInteractor.cycleRepeatModeCalled).isTrue()
    }

    @Test
    fun `idle playback state shows loading`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = PlayerScreenViewModel.Interactor.PlaybackState.Idle
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    private fun createPlaybackState(
        trackId: String,
        trackName: String = "Test Track",
        albumId: String = "album-1",
        albumName: String = "Test Album",
        albumImageUrl: String? = null,
        artists: List<ArtistInfo> = emptyList(),
        trackDurationSec: Int = 180,
        isPlaying: Boolean = false,
    ) = PlayerScreenViewModel.Interactor.PlaybackState.Loaded(
        isPlaying = isPlaying,
        trackId = trackId,
        trackName = trackName,
        albumId = albumId,
        albumName = albumName,
        albumImageUrl = albumImageUrl,
        artists = artists,
        trackPercent = 0f,
        trackProgressSec = 0,
        trackDurationSec = trackDurationSec,
        hasNextTrack = false,
        hasPreviousTrack = false,
        volume = 0.5f,
        isMuted = false,
        shuffleEnabled = false,
        repeatMode = RepeatModeUi.OFF,
        playerError = null,
    )

    private class FakeInteractor : PlayerScreenViewModel.Interactor {
        val playbackStateFlow = MutableStateFlow<PlayerScreenViewModel.Interactor.PlaybackState?>(null)

        var togglePlayPauseCalled = false
        var skipToNextCalled = false
        var skipToPreviousCalled = false
        var lastSeekPercent: Float? = null
        var lastVolume: Float? = null
        var toggleMuteCalled = false
        var toggleShuffleCalled = false
        var cycleRepeatModeCalled = false
        var retryCalled = false

        override fun getPlaybackState(): Flow<PlayerScreenViewModel.Interactor.PlaybackState?> =
            playbackStateFlow

        override fun togglePlayPause() {
            togglePlayPauseCalled = true
        }

        override fun skipToNext() {
            skipToNextCalled = true
        }

        override fun skipToPrevious() {
            skipToPreviousCalled = true
        }

        override fun seekToPercent(percent: Float) {
            lastSeekPercent = percent
        }

        override fun setVolume(volume: Float) {
            lastVolume = volume
        }

        override fun toggleMute() {
            toggleMuteCalled = true
        }

        override fun toggleShuffle() {
            toggleShuffleCalled = true
        }

        override fun cycleRepeatMode() {
            cycleRepeatModeCalled = true
        }

        override fun retry() {
            retryCalled = true
        }

        override fun getRemoteDeviceName(): Flow<String?> = flowOf(null)

        override fun getHasOtherDeviceConnected(): Flow<Boolean> = flowOf(false)

        override fun exitRemoteMode() {}
    }
}
