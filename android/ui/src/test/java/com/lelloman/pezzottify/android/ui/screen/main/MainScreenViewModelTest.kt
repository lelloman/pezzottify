package com.lelloman.pezzottify.android.ui.screen.main

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
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
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var viewModel: MainScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        fakeContentResolver = FakeContentResolver()
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
            contentResolver = fakeContentResolver,
        )
    }

    @Test
    fun `initial state has hidden bottom player`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isFalse()
    }

    @Test
    fun `bottom player becomes visible when playback state is loaded`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved(
                "track-1",
                Track("track-1", "Test Track", "album-1", emptyList(), 180)
            )
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved(
                "album-1",
                Album("album-1", "Test Album", 1609459200L, "http://img.com/1.jpg", emptyList())
            )
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.5f,
            nextTrackId = null,
            previousTrackId = null,
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isTrue()
        assertThat(viewModel.state.value.bottomPlayer.trackId).isEqualTo("track-1")
        assertThat(viewModel.state.value.bottomPlayer.isPlaying).isTrue()
        assertThat(viewModel.state.value.bottomPlayer.trackPercent).isEqualTo(0.5f)
    }

    @Test
    fun `bottom player hides when playback state becomes idle`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved("track-1", Track("track-1", "Test", "album-1", emptyList(), 180))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved("album-1", Album("album-1", "Album", 1609459200L, null, emptyList()))
        )

        createViewModel()
        advanceUntilIdle()

        // First show bottom player
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.3f,
            nextTrackId = null,
            previousTrackId = null,
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isTrue()

        // Then go idle
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Idle
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.isVisible).isFalse()
    }

    @Test
    fun `bottom player resolves track name from content resolver`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved(
                "track-1",
                Track(
                    id = "track-1",
                    name = "Amazing Song",
                    albumId = "album-1",
                    artists = listOf(ArtistInfo("artist-1", "Great Artist")),
                    durationSeconds = 240
                )
            )
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved(
                "album-1",
                Album("album-1", "Best Album", 1609459200L, "http://img.com/cover.jpg", listOf("artist-1"))
            )
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = false,
            trackId = "track-1",
            trackPercent = 0f,
            nextTrackId = null,
            previousTrackId = null,
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.trackName).isEqualTo("Amazing Song")
        assertThat(viewModel.state.value.bottomPlayer.artists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.artists[0].name).isEqualTo("Great Artist")
    }

    @Test
    fun `bottom player resolves album info from content resolver`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved("track-1", Track("track-1", "Song", "album-1", emptyList(), 180))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved(
                "album-1",
                Album(
                    id = "album-1",
                    name = "Epic Album",
                    date = 1609459200L,
                    imageUrl = "http://cdn.com/album-cover.jpg",
                    artistsIds = emptyList()
                )
            )
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = false,
            trackId = "track-1",
            trackPercent = 0f,
            nextTrackId = null,
            previousTrackId = null,
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
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved("track-1", Track("track-1", "Current Song", "album-1", emptyList(), 180))
        )
        fakeContentResolver.trackResults["track-2"] = flowOf(
            Content.Resolved(
                "track-2",
                Track("track-2", "Next Song", "album-1", listOf(ArtistInfo("artist-1", "Next Artist")), 200)
            )
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved("album-1", Album("album-1", "Album", 1609459200L, null, emptyList()))
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.5f,
            nextTrackId = "track-2",
            previousTrackId = null,
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.nextTrackName).isEqualTo("Next Song")
        assertThat(viewModel.state.value.bottomPlayer.nextTrackArtists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.nextTrackArtists[0].name).isEqualTo("Next Artist")
    }

    @Test
    fun `bottom player shows previous track info when available`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved("track-1", Track("track-1", "Current Song", "album-1", emptyList(), 180))
        )
        fakeContentResolver.trackResults["track-0"] = flowOf(
            Content.Resolved(
                "track-0",
                Track("track-0", "Previous Song", "album-1", listOf(ArtistInfo("artist-1", "Previous Artist")), 150)
            )
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved("album-1", Album("album-1", "Album", 1609459200L, null, emptyList()))
        )

        createViewModel()
        advanceUntilIdle()

        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.5f,
            nextTrackId = null,
            previousTrackId = "track-0",
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.previousTrackName).isEqualTo("Previous Song")
        assertThat(viewModel.state.value.bottomPlayer.previousTrackArtists).hasSize(1)
        assertThat(viewModel.state.value.bottomPlayer.previousTrackArtists[0].name).isEqualTo("Previous Artist")
    }

    @Test
    fun `bottom player clears next track info when no next track`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(
            Content.Resolved("track-1", Track("track-1", "Song 1", "album-1", emptyList(), 180))
        )
        fakeContentResolver.trackResults["track-2"] = flowOf(
            Content.Resolved("track-2", Track("track-2", "Song 2", "album-1", emptyList(), 180))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved("album-1", Album("album-1", "Album", 1609459200L, null, emptyList()))
        )

        createViewModel()
        advanceUntilIdle()

        // First with next track
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.5f,
            nextTrackId = "track-2",
            previousTrackId = null,
        )
        advanceUntilIdle()

        assertThat(viewModel.state.value.bottomPlayer.nextTrackName).isEqualTo("Song 2")

        // Then without next track
        fakeInteractor.playbackStateFlow.value = MainScreenViewModel.Interactor.PlaybackState.Loaded(
            isPlaying = true,
            trackId = "track-1",
            trackPercent = 0.7f,
            nextTrackId = null,
            previousTrackId = null,
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
    }

    private class FakeContentResolver : ContentResolver {
        val trackResults = mutableMapOf<String, Flow<Content<Track>>>()
        val albumResults = mutableMapOf<String, Flow<Content<Album>>>()

        override fun resolveSearchResult(
            itemId: String,
            itemType: SearchScreenViewModel.SearchedItemType
        ): Flow<Content<SearchResultContent>> = flowOf(Content.Loading(itemId))

        override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
            flowOf(Content.Loading(artistId))

        override fun resolveAlbum(albumId: String): Flow<Content<Album>> =
            albumResults[albumId] ?: flowOf(Content.Loading(albumId))

        override fun resolveTrack(trackId: String): Flow<Content<Track>> =
            trackResults[trackId] ?: flowOf(Content.Loading(trackId))

        override fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>> =
            flowOf(Content.Loading(artistId))
    }
}
