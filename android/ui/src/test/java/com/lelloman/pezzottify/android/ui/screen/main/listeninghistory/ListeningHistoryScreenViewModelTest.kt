package com.lelloman.pezzottify.android.ui.screen.main.listeninghistory

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
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
class ListeningHistoryScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var viewModel: ListeningHistoryScreenViewModel

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
        viewModel = ListeningHistoryScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            coroutineContext = testDispatcher,
        )
    }

    @Test
    fun `initial state has no error and no events`() = runTest {
        fakeInteractor.eventsToReturn = Result.success(emptyList())

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.errorRes).isNull()
        assertThat(viewModel.state.value.events).isEmpty()
    }

    @Test
    fun `state shows events after successful load`() = runTest {
        val events = listOf(
            createEvent(id = 1, trackId = "track-1"),
            createEvent(id = 2, trackId = "track-2"),
        )
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.events).hasSize(2)
        assertThat(viewModel.state.value.events[0].trackId).isEqualTo("track-1")
        assertThat(viewModel.state.value.events[1].trackId).isEqualTo("track-2")
    }

    @Test
    fun `state shows network error after network failure`() = runTest {
        fakeInteractor.eventsToReturn = Result.failure(
            ListeningHistoryException(ListeningHistoryErrorType.Network)
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.events).isEmpty()
        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.listening_history_error_network)
    }

    @Test
    fun `state shows unauthorized error after auth failure`() = runTest {
        fakeInteractor.eventsToReturn = Result.failure(
            ListeningHistoryException(ListeningHistoryErrorType.Unauthorized)
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.listening_history_error_unauthorized)
    }

    @Test
    fun `state shows unknown error for other failures`() = runTest {
        fakeInteractor.eventsToReturn = Result.failure(Exception("Some other error"))

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.listening_history_error_unknown)
    }

    @Test
    fun `hasMorePages is true when page is full`() = runTest {
        // Return exactly 50 items (the page size)
        val events = (1..50).map { createEvent(id = it.toLong(), trackId = "track-$it") }
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasMorePages).isTrue()
    }

    @Test
    fun `hasMorePages is false when page is not full`() = runTest {
        val events = listOf(
            createEvent(id = 1, trackId = "track-1"),
            createEvent(id = 2, trackId = "track-2"),
        )
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasMorePages).isFalse()
    }

    @Test
    fun `refresh reloads data`() = runTest {
        val initialEvents = listOf(createEvent(id = 1, trackId = "track-1"))
        fakeInteractor.eventsToReturn = Result.success(initialEvents)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.events).hasSize(1)

        // Change what the interactor returns
        val newEvents = listOf(
            createEvent(id = 1, trackId = "track-1"),
            createEvent(id = 2, trackId = "track-2"),
            createEvent(id = 3, trackId = "track-3"),
        )
        fakeInteractor.eventsToReturn = Result.success(newEvents)

        viewModel.refresh()
        advanceUntilIdle()

        assertThat(viewModel.state.value.events).hasSize(3)
    }

    @Test
    fun `loadMore appends events`() = runTest {
        // First page
        val firstPage = (1..50).map { createEvent(id = it.toLong(), trackId = "track-$it") }
        fakeInteractor.eventsToReturn = Result.success(firstPage)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.events).hasSize(50)
        assertThat(fakeInteractor.lastOffset).isEqualTo(0)

        // Second page
        val secondPage = (51..60).map { createEvent(id = it.toLong(), trackId = "track-$it") }
        fakeInteractor.eventsToReturn = Result.success(secondPage)

        viewModel.loadMore()
        advanceUntilIdle()

        assertThat(viewModel.state.value.events).hasSize(60)
        assertThat(fakeInteractor.lastOffset).isEqualTo(50)
        assertThat(viewModel.state.value.hasMorePages).isFalse() // less than 50 items
    }

    @Test
    fun `loadMore does nothing when already loading`() = runTest {
        val events = (1..50).map { createEvent(id = it.toLong(), trackId = "track-$it") }
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        // Don't advance - still loading

        val callCountBefore = fakeInteractor.callCount
        viewModel.loadMore()

        // Should not have called again since we're still loading
        assertThat(fakeInteractor.callCount).isEqualTo(callCountBefore)
    }

    @Test
    fun `loadMore does nothing when no more pages`() = runTest {
        val events = listOf(createEvent(id = 1, trackId = "track-1"))
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasMorePages).isFalse()

        val callCountBefore = fakeInteractor.callCount
        viewModel.loadMore()
        advanceUntilIdle()

        // Should not have called again
        assertThat(fakeInteractor.callCount).isEqualTo(callCountBefore)
    }

    @Test
    fun `completed events are marked correctly`() = runTest {
        val events = listOf(
            createEvent(id = 1, trackId = "track-1", completed = true),
            createEvent(id = 2, trackId = "track-2", completed = false),
        )
        fakeInteractor.eventsToReturn = Result.success(events)

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.events[0].completed).isTrue()
        assertThat(viewModel.state.value.events[1].completed).isFalse()
    }

    private fun createEvent(
        id: Long,
        trackId: String,
        startedAt: Long = 1700000000L,
        durationSeconds: Int = 180,
        trackDurationSeconds: Int = 200,
        completed: Boolean = false,
        playbackContext: String? = "album",
        clientType: String? = "android",
    ) = UiListeningEvent(
        id = id,
        trackId = trackId,
        startedAt = startedAt,
        durationSeconds = durationSeconds,
        trackDurationSeconds = trackDurationSeconds,
        completed = completed,
        playbackContext = playbackContext,
        clientType = clientType,
    )

    private class FakeInteractor : ListeningHistoryScreenViewModel.Interactor {
        var eventsToReturn: Result<List<UiListeningEvent>> = Result.success(emptyList())
        var lastLimit: Int = 0
        var lastOffset: Int = 0
        var callCount: Int = 0

        override suspend fun getListeningEvents(limit: Int, offset: Int): Result<List<UiListeningEvent>> {
            callCount++
            lastLimit = limit
            lastOffset = offset
            return eventsToReturn
        }
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
