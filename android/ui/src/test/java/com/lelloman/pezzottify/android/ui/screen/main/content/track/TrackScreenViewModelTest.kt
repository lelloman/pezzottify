package com.lelloman.pezzottify.android.ui.screen.main.content.track

import androidx.navigation.NavController
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
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
class TrackScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var navController: NavController
    private lateinit var viewModel: TrackScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        fakeContentResolver = FakeContentResolver()
        navController = mockk(relaxed = true)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel(trackId: String = "track-1") {
        viewModel = TrackScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            trackId = trackId,
            navController = navController,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state shows error when track fails to load`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Error("track-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isError).isTrue()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `state shows track from content resolver`() = runTest {
        val track = Track(
            id = "track-1",
            name = "Amazing Song",
            albumId = "album-1",
            artists = listOf(ArtistInfo("artist-1", "Great Artist")),
            durationSeconds = 240
        )
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isError).isFalse()
        assertThat(viewModel.state.value.track).isEqualTo(track)
        assertThat(viewModel.state.value.track?.name).isEqualTo("Amazing Song")
    }

    @Test
    fun `state shows album from content resolver`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        val album = Album(
            id = "album-1",
            name = "Epic Album",
            date = 1609459200,
            imageUrl = "http://img.com/cover.jpg",
            artistsIds = listOf("artist-1")
        )
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.album).isEqualTo(album)
        assertThat(viewModel.state.value.album?.name).isEqualTo("Epic Album")
    }

    @Test
    fun `state shows current playing track id`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        fakeInteractor.currentPlayingTrackIdFlow.value = "track-1"
        advanceUntilIdle()

        assertThat(viewModel.state.value.currentPlayingTrackId).isEqualTo("track-1")
    }

    @Test
    fun `state shows liked status`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))
        fakeInteractor.likedContentIds.add("track-1")

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLiked).isTrue()
    }

    @Test
    fun `clickOnPlayTrack calls interactor playTrack`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        viewModel.clickOnPlayTrack()

        assertThat(fakeInteractor.lastPlayedAlbumId).isEqualTo("album-1")
        assertThat(fakeInteractor.lastPlayedTrackId).isEqualTo("track-1")
    }

    @Test
    fun `clickOnLike calls interactor toggleLike`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        viewModel.clickOnLike()

        assertThat(fakeInteractor.lastToggleLikeContentId).isEqualTo("track-1")
        assertThat(fakeInteractor.lastToggleLikeCurrentlyLiked).isFalse()
    }

    @Test
    fun `logs viewed track when loaded`() = runTest {
        val track = Track("track-1", "Song", "album-1", emptyList(), 180)
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Resolved("track-1", track))
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Loading("album-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.loggedViewedTrackId).isEqualTo("track-1")
    }

    @Test
    fun `does not log viewed track when loading`() = runTest {
        fakeContentResolver.trackResults["track-1"] = flowOf(Content.Loading("track-1"))

        createViewModel("track-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.loggedViewedTrackId).isNull()
    }

    private class FakeInteractor : TrackScreenViewModel.Interactor {
        val currentPlayingTrackIdFlow = MutableStateFlow<String?>(null)
        val likedContentIds = mutableSetOf<String>()

        var lastPlayedAlbumId: String? = null
        var lastPlayedTrackId: String? = null
        var loggedViewedTrackId: String? = null
        var lastToggleLikeContentId: String? = null
        var lastToggleLikeCurrentlyLiked: Boolean? = null

        override fun playTrack(albumId: String, trackId: String) {
            lastPlayedAlbumId = albumId
            lastPlayedTrackId = trackId
        }

        override fun logViewedTrack(trackId: String) {
            loggedViewedTrackId = trackId
        }

        override fun getCurrentPlayingTrackId(): Flow<String?> = currentPlayingTrackIdFlow

        override fun isLiked(contentId: String): Flow<Boolean> =
            MutableStateFlow(likedContentIds.contains(contentId))

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            lastToggleLikeContentId = contentId
            lastToggleLikeCurrentlyLiked = currentlyLiked
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

        override fun buildImageUrl(displayImageId: String): String =
            "http://example.com/image/$displayImageId"
    }
}
