package com.lelloman.pezzottify.android.ui.screen.main.search

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SearchScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var viewModel: SearchScreenViewModel

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
        viewModel = SearchScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            coroutineContext = testDispatcher,
        )
    }

    @Test
    fun `initial state has empty query`() = runTest {
        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.query).isEmpty()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `updateQuery updates state and triggers search`() = runTest {
        fakeInteractor.searchResults = Result.success(
            listOf("album-1" to SearchScreenViewModel.SearchedItemType.Album)
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.updateQuery("test query")

        assertThat(viewModel.state.value.query).isEqualTo("test query")
        assertThat(viewModel.state.value.isLoading).isTrue()

        // Wait for debounce (400ms) and search to complete
        advanceTimeBy(500)
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.searchResults).isNotNull()
    }

    @Test
    fun `toggleFilter adds filter when not present`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.toggleFilter(SearchFilter.Album)

        assertThat(viewModel.state.value.selectedFilters).contains(SearchFilter.Album)
    }

    @Test
    fun `toggleFilter removes filter when present`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.toggleFilter(SearchFilter.Album)
        assertThat(viewModel.state.value.selectedFilters).contains(SearchFilter.Album)

        viewModel.toggleFilter(SearchFilter.Album)
        assertThat(viewModel.state.value.selectedFilters).doesNotContain(SearchFilter.Album)
    }

    @Test
    fun `toggleFilter triggers re-search when query is not empty`() = runTest {
        fakeInteractor.searchResults = Result.success(emptyList())
        createViewModel()
        advanceUntilIdle()

        viewModel.updateQuery("test")
        advanceTimeBy(500)
        advanceUntilIdle()

        val searchCountBefore = fakeInteractor.searchCallCount

        viewModel.toggleFilter(SearchFilter.Artist)
        advanceTimeBy(500)
        advanceUntilIdle()

        assertThat(fakeInteractor.searchCallCount).isGreaterThan(searchCountBefore)
    }

    @Test
    fun `clickOnArtistSearchResult emits navigation event and logs history`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<SearchScreensEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        // Set a query first so history is logged
        viewModel.updateQuery("artist name")
        advanceTimeBy(500)
        advanceUntilIdle()

        viewModel.clickOnArtistSearchResult("artist-123")
        advanceUntilIdle()

        assertThat(events).contains(SearchScreensEvents.NavigateToArtistScreen("artist-123"))
        assertThat(fakeInteractor.lastLoggedSearchEntry).isNotNull()
        assertThat(fakeInteractor.lastLoggedSearchEntry?.contentId).isEqualTo("artist-123")
        assertThat(fakeInteractor.lastLoggedSearchEntry?.contentType)
            .isEqualTo(SearchScreenViewModel.SearchHistoryEntryType.Artist)

        job.cancel()
    }

    @Test
    fun `clickOnAlbumSearchResult emits navigation event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<SearchScreensEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnAlbumSearchResult("album-456")
        advanceUntilIdle()

        assertThat(events).contains(SearchScreensEvents.NavigateToAlbumScreen("album-456"))

        job.cancel()
    }

    @Test
    fun `clickOnTrackSearchResult emits navigation event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<SearchScreensEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnTrackSearchResult("track-789")
        advanceUntilIdle()

        assertThat(events).contains(SearchScreensEvents.NavigateToTrackScreen("track-789"))

        job.cancel()
    }

    @Test
    fun `clickOnRecentlyViewedItem emits correct navigation event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<SearchScreensEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnRecentlyViewedItem("artist-1", ViewedContentType.Artist)
        advanceUntilIdle()
        assertThat(events.last()).isEqualTo(SearchScreensEvents.NavigateToArtistScreen("artist-1"))

        viewModel.clickOnRecentlyViewedItem("album-1", ViewedContentType.Album)
        advanceUntilIdle()
        assertThat(events.last()).isEqualTo(SearchScreensEvents.NavigateToAlbumScreen("album-1"))

        viewModel.clickOnRecentlyViewedItem("track-1", ViewedContentType.Track)
        advanceUntilIdle()
        assertThat(events.last()).isEqualTo(SearchScreensEvents.NavigateToTrackScreen("track-1"))

        job.cancel()
    }

    @Test
    fun `empty query clears search results`() = runTest {
        fakeInteractor.searchResults = Result.success(
            listOf("album-1" to SearchScreenViewModel.SearchedItemType.Album)
        )
        createViewModel()
        advanceUntilIdle()

        viewModel.updateQuery("test")
        advanceTimeBy(500)
        advanceUntilIdle()

        assertThat(viewModel.state.value.searchResults).isNotNull()

        viewModel.updateQuery("")
        advanceUntilIdle()

        assertThat(viewModel.state.value.searchResults).isNull()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    private class FakeInteractor : SearchScreenViewModel.Interactor {
        val recentlyViewedFlow = MutableStateFlow<List<SearchScreenViewModel.RecentlyViewedContent>>(emptyList())
        val searchHistoryFlow = MutableStateFlow<List<SearchScreenViewModel.SearchHistoryEntry>>(emptyList())

        var searchResults: Result<List<Pair<String, SearchScreenViewModel.SearchedItemType>>> = Result.success(emptyList())

        var searchCallCount = 0
        var lastLoggedSearchEntry: LoggedEntry? = null

        data class LoggedEntry(val query: String, val contentType: SearchScreenViewModel.SearchHistoryEntryType, val contentId: String)

        override suspend fun search(
            query: String,
            filters: List<SearchScreenViewModel.InteractorSearchFilter>?
        ): Result<List<Pair<String, SearchScreenViewModel.SearchedItemType>>> {
            searchCallCount++
            return searchResults
        }

        override suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<SearchScreenViewModel.RecentlyViewedContent>> =
            recentlyViewedFlow

        override fun getSearchHistoryEntries(maxCount: Int): Flow<List<SearchScreenViewModel.SearchHistoryEntry>> =
            searchHistoryFlow

        override fun logSearchHistoryEntry(
            query: String,
            contentType: SearchScreenViewModel.SearchHistoryEntryType,
            contentId: String
        ) {
            lastLoggedSearchEntry = LoggedEntry(query, contentType, contentId)
        }

        override suspend fun getWhatsNew(limit: Int): Result<List<SearchScreenViewModel.WhatsNewBatchData>> =
            Result.success(emptyList())
    }

    private class FakeContentResolver : ContentResolver {
        override fun resolveSearchResult(
            itemId: String,
            itemType: SearchScreenViewModel.SearchedItemType
        ): Flow<Content<SearchResultContent>> = flowOf(Content.Loading(itemId))

        override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
            flowOf(Content.Loading(artistId))

        override fun resolveAlbum(albumId: String): Flow<Content<Album>> =
            flowOf(Content.Loading(albumId))

        override fun resolveTrack(trackId: String): Flow<Content<Track>> =
            flowOf(Content.Loading(trackId))

        override fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>> =
            flowOf(Content.Loading(artistId))
    }
}
