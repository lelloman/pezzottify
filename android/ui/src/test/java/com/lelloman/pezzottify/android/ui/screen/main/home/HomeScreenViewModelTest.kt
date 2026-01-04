package com.lelloman.pezzottify.android.ui.screen.main.home

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.component.ConnectionState
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class HomeScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var viewModel: HomeScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        fakeContentResolver = FakeContentResolver()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel() {
        viewModel = HomeScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            coroutineContext = testDispatcher,
        )
    }

    @Test
    fun `initial state has default values`() = runTest {
        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.userName).isEqualTo("TestUser")
        assertThat(viewModel.state.value.connectionState).isEqualTo(ConnectionState.Disconnected)
    }

    @Test
    fun `loads user name from interactor`() = runTest {
        fakeInteractor.setUserName("Alice")

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.userName).isEqualTo("Alice")
    }

    @Test
    fun `loads popular content from interactor`() = runTest {
        val popularContent = PopularContentState(
            albums = listOf(
                PopularAlbumState("album-1", "Album One", "http://img.com/1", listOf("Artist A"))
            ),
            artists = listOf(
                PopularArtistState("artist-1", "Artist One", "http://img.com/2")
            ),
        )
        fakeInteractor.setPopularContent(popularContent)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.popularContent).isEqualTo(popularContent)
    }

    @Test
    fun `loads recently viewed content from interactor`() = runTest {
        val recentlyViewed = listOf(
            HomeScreenState.RecentlyViewedContent("artist-1", ViewedContentType.Artist),
            HomeScreenState.RecentlyViewedContent("album-1", ViewedContentType.Album),
        )
        fakeInteractor.setRecentlyViewedContent(recentlyViewed)

        // Set up content resolver to return resolved content
        fakeContentResolver.artistResults["artist-1"] = flowOf(
            Content.Resolved("artist-1", Artist("artist-1", "Test Artist", null, emptyList()))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(
            Content.Resolved("album-1", Album("album-1", "Test Album", 1609459200L, null, listOf("artist-1")))
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.recentlyViewedContent).isNotNull()
        assertThat(viewModel.state.value.recentlyViewedContent).hasSize(2)
    }

    @Test
    fun `updates connection state from interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.connectionState).isEqualTo(ConnectionState.Disconnected)

        // Update connection state
        fakeInteractor.connectionStateFlow.value = ConnectionState.Connecting
        advanceUntilIdle()

        assertThat(viewModel.state.value.connectionState).isEqualTo(ConnectionState.Connecting)

        // Update to connected
        fakeInteractor.connectionStateFlow.value = ConnectionState.Connected(1, "1.0.0")
        advanceUntilIdle()

        assertThat(viewModel.state.value.connectionState).isEqualTo(ConnectionState.Connected(1, "1.0.0"))
    }

    @Test
    fun `clickOnProfile emits NavigateToProfileScreen event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnProfile()
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToProfileScreen)

        job.cancel()
    }

    @Test
    fun `clickOnSettings emits NavigateToSettingsScreen event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnSettings()
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToSettingsScreen)

        job.cancel()
    }

    @Test
    fun `clickOnRecentlyViewedItem with Artist emits NavigateToArtist`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnRecentlyViewedItem("artist-123", ViewedContentType.Artist)
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToArtist("artist-123"))

        job.cancel()
    }

    @Test
    fun `clickOnRecentlyViewedItem with Album emits NavigateToAlbum`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnRecentlyViewedItem("album-456", ViewedContentType.Album)
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToAlbum("album-456"))

        job.cancel()
    }

    @Test
    fun `clickOnRecentlyViewedItem with Track emits NavigateToTrack`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnRecentlyViewedItem("track-789", ViewedContentType.Track)
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToTrack("track-789"))

        job.cancel()
    }

    @Test
    fun `clickOnPopularAlbum emits NavigateToAlbum`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnPopularAlbum("popular-album-1")
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToAlbum("popular-album-1"))

        job.cancel()
    }

    @Test
    fun `clickOnPopularArtist emits NavigateToArtist`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<HomeScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnPopularArtist("popular-artist-1")
        advanceUntilIdle()

        assertThat(events).containsExactly(HomeScreenEvents.NavigateToArtist("popular-artist-1"))

        job.cancel()
    }

    private class FakeInteractor : HomeScreenViewModel.Interactor {
        private var _userName = "TestUser"
        private var _popularContent: PopularContentState? = null
        private var _recentlyViewedContent: List<HomeScreenState.RecentlyViewedContent> = emptyList()
        val connectionStateFlow = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)

        fun setUserName(name: String) { _userName = name }
        fun setPopularContent(content: PopularContentState?) { _popularContent = content }
        fun setRecentlyViewedContent(content: List<HomeScreenState.RecentlyViewedContent>) { _recentlyViewedContent = content }

        override fun connectionState(scope: CoroutineScope): StateFlow<ConnectionState> =
            connectionStateFlow

        override suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<HomeScreenState.RecentlyViewedContent>> =
            flowOf(_recentlyViewedContent)

        override fun getUserName(): String = _userName

        override suspend fun getPopularContent(): PopularContentState? = _popularContent
    }

    private class FakeContentResolver : ContentResolver {
        val artistResults = mutableMapOf<String, Flow<Content<Artist>>>()
        val albumResults = mutableMapOf<String, Flow<Content<Album>>>()
        val trackResults = mutableMapOf<String, Flow<Content<Track>>>()

        override fun resolveSearchResult(
            itemId: String,
            itemType: SearchScreenViewModel.SearchedItemType
        ): Flow<Content<SearchResultContent>> = flowOf(Content.Loading(itemId))

        override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
            artistResults[artistId] ?: flowOf(Content.Loading(artistId))

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
