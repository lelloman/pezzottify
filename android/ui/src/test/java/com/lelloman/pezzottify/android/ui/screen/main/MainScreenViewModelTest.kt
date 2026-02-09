package com.lelloman.pezzottify.android.ui.screen.main

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import io.mockk.every
import io.mockk.mockk
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
class MainScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var viewModel: MainScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk<LoggerFactory>()
        every { loggerFactory.getValue(any(), any()) } returns mockLogger
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel() {
        viewModel = MainScreenViewModel(
            interactor = fakeInteractor,
            loggerFactory = loggerFactory,
        )
    }

    @Test
    fun `initial state has hidden bottom player`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isFalse()
    }

    @Test
    fun `bottom player becomes visible when playback state is loaded`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Test Track",
            albumName = "Test Album",
            albumImageUrl = "http://img.com/1.jpg",
            artists = listOf(ArtistInfo("artist-1", "Test Artist")),
            trackPercent = 0.5f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isTrue()
        assertThat(viewModel.state.value.bottomPlayer.trackId).isEqualTo("track-1")
        assertThat(viewModel.state.value.bottomPlayer.isPlaying).isTrue()
        assertThat(viewModel.state.value.bottomPlayer.trackPercent).isEqualTo(0.5f)
    }

    @Test
    fun `bottom player hides when playback state becomes idle`() = runTest {
        createViewModel()
        advanceUntilIdle()

        // First show bottom player
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Test Track",
            albumName = "Test Album",
            albumImageUrl = null,
            artists = emptyList(),
            trackPercent = 0.3f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isTrue()

        // Then go idle
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Idle
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isFalse()
    }

    @Test
    fun `bottom player displays track name and artists from playback state`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = false,
            trackId = "track-1",
            trackName = "Amazing Song",
            albumName = "Best Album",
            albumImageUrl = "http://img.com/cover.jpg",
            artists = listOf(ArtistInfo("artist-1", "Great Artist")),
            trackPercent = 0f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.trackName).isEqualTo("Amazing Song")
        assertThat(viewModel.state.value.bottomPlayer.artists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.artists[0].name).isEqualTo("Great Artist")
    }

    @Test
    fun `bottom player displays album info from playback state`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = false,
            trackId = "track-1",
            trackName = "Song",
            albumName = "Epic Album",
            albumImageUrl = "http://cdn.com/album-cover.jpg",
            artists = emptyList(),
            trackPercent = 0f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.albumName).isEqualTo("Epic Album")
        assertThat(viewModel.state.value.bottomPlayer.albumImageUrl).isEqualTo("http://cdn.com/album-cover.jpg")
    }

    @Test
    fun `clickOnPlayPause calls interactor`() = runTest {
        createViewModel()

        viewModel.clickOnPlayPause()

        assertThat(fakeInteractor.playPauseCalled).isTrue()
    }

    @Test
    fun `clickOnSkipToNext calls interactor`() = runTest {
        createViewModel()

        viewModel.clickOnSkipToNext()

        assertThat(fakeInteractor.skipToNextCalled).isTrue()
    }

    @Test
    fun `clickOnSkipToPrevious calls interactor`() = runTest {
        createViewModel()

        viewModel.clickOnSkipToPrevious()

        assertThat(fakeInteractor.skipToPreviousCalled).isTrue()
    }

    @Test
    fun `bottom player shows next track info when available`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Current Song",
            albumName = "Album",
            albumImageUrl = null,
            artists = emptyList(),
            trackPercent = 0.5f,
            nextTrackName = "Next Song",
            nextTrackArtists = listOf(ArtistInfo("artist-1", "Next Artist")),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.nextTrackName).isEqualTo("Next Song")
        assertThat(viewModel.state.value.bottomPlayer.nextTrackArtists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.nextTrackArtists[0].name).isEqualTo("Next Artist")
    }

    @Test
    fun `bottom player shows previous track info when available`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Current Song",
            albumName = "Album",
            albumImageUrl = null,
            artists = emptyList(),
            trackPercent = 0.5f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = "Previous Song",
            previousTrackArtists = listOf(ArtistInfo("artist-1", "Previous Artist")),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.previousTrackName).isEqualTo("Previous Song")
        assertThat(viewModel.state.value.bottomPlayer.previousTrackArtists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.previousTrackArtists[0].name).isEqualTo("Previous Artist")
    }

    @Test
    fun `bottom player clears next track info when no next track`() = runTest {
        createViewModel()
        advanceUntilIdle()

        // First with next track
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Song 1",
            albumName = "Album",
            albumImageUrl = null,
            artists = emptyList(),
            trackPercent = 0.5f,
            nextTrackName = "Song 2",
            nextTrackArtists = listOf(ArtistInfo("artist-1", "Artist")),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.nextTrackName).isEqualTo("Song 2")

        // Then without next track
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackName = "Song 1",
            albumName = "Album",
            albumImageUrl = null,
            artists = emptyList(),
            trackPercent = 0.7f,
            nextTrackName = null,
            nextTrackArtists = emptyList(),
            previousTrackName = null,
            previousTrackArtists = emptyList(),
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.nextTrackName).isNull()
        assertThat(viewModel.state.value.bottomPlayer.nextTrackArtists).isEmpty()
    }

    private class FakeInteractor : MainScreenViewModel.Interactor {
        val playbackStateFlow = MutableStateFlow<MainScreenViewModel.Interactor.PlaybackState?>(null)
        val notificationUnreadCountFlow = MutableStateFlow(0)

        var playPauseCalled = false
        var skipToNextCalled = false
        var skipToPreviousCalled = false

        override fun getPlaybackState(): Flow<MainScreenViewModel.Interactor.PlaybackState?> = playbackStateFlow

        override fun getNotificationUnreadCount(): Flow<Int> = notificationUnreadCountFlow

        override fun clickOnPlayPause() {
            playPauseCalled = true
        }

        override fun clickOnSkipToNext() {
            skipToNextCalled = true
        }

        override fun clickOnSkipToPrevious() {
            skipToPreviousCalled = true
        }

        override fun getRemoteDeviceName(): Flow<String?> = flowOf(null)

        override fun getHasOtherDeviceConnected(): Flow<Boolean> = flowOf(false)
    }
}
