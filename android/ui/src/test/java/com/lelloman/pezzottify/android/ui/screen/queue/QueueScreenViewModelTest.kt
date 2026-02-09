package com.lelloman.pezzottify.android.ui.screen.queue

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
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
class QueueScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: QueueScreenViewModel

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
        viewModel = QueueScreenViewModel(
            interactor = fakeInteractor,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state shows error when queue state is null`() = runTest {
        fakeInteractor.queueStateFlow.value = null

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isError).isTrue()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `state shows tracks from queue`() = runTest {
        val queueState = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(
                createQueueTrack("track-1"),
                createQueueTrack("track-2"),
                createQueueTrack("track-3"),
            ),
            currentIndex = 1,
            contextName = "Test Album",
            contextType = QueueContextType.Album,
            canSaveAsPlaylist = false,
        )
        fakeInteractor.queueStateFlow.value = queueState

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isError).isFalse()
        assertThat(viewModel.state.value.tracks).hasSize(3)
        assertThat(viewModel.state.value.currentTrackIndex).isEqualTo(1)
    }

    @Test
    fun `state shows Album context type for album playlist`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(createQueueTrack("track-1")),
            currentIndex = 0,
            contextName = "album-1",
            contextType = QueueContextType.Album,
            canSaveAsPlaylist = false,
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.Album)
        assertThat(viewModel.state.value.contextName).isEqualTo("album-1")
        assertThat(viewModel.state.value.canSaveAsPlaylist).isFalse()
    }

    @Test
    fun `state shows UserPlaylist context type for user playlist`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(createQueueTrack("track-1")),
            currentIndex = 0,
            contextName = "playlist-1",
            contextType = QueueContextType.UserPlaylist,
            canSaveAsPlaylist = true,
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.UserPlaylist)
        assertThat(viewModel.state.value.contextName).isEqualTo("playlist-1")
        assertThat(viewModel.state.value.canSaveAsPlaylist).isTrue()
    }

    @Test
    fun `state shows UserMix context type for user mix`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(createQueueTrack("track-1")),
            currentIndex = 0,
            contextName = "",
            contextType = QueueContextType.UserMix,
            canSaveAsPlaylist = true,
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.UserMix)
        assertThat(viewModel.state.value.canSaveAsPlaylist).isTrue()
    }

    @Test
    fun `clickOnTrack calls interactor playTrackAtIndex`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(createQueueTrack("track-1"), createQueueTrack("track-2")),
            currentIndex = 0,
            contextName = "",
            contextType = QueueContextType.UserMix,
            canSaveAsPlaylist = true,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.clickOnTrack(1)

        assertThat(fakeInteractor.lastPlayedIndex).isEqualTo(1)
    }

    @Test
    fun `moveTrack calls interactor moveTrack`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(
                createQueueTrack("track-1"),
                createQueueTrack("track-2"),
                createQueueTrack("track-3"),
            ),
            currentIndex = 0,
            contextName = "",
            contextType = QueueContextType.UserMix,
            canSaveAsPlaylist = true,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.moveTrack(0, 2)

        assertThat(fakeInteractor.lastMoveFrom).isEqualTo(0)
        assertThat(fakeInteractor.lastMoveTo).isEqualTo(2)
    }

    @Test
    fun `removeTrack calls interactor removeTrack`() = runTest {
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(createQueueTrack("track-1"), createQueueTrack("track-2")),
            currentIndex = 0,
            contextName = "",
            contextType = QueueContextType.UserMix,
            canSaveAsPlaylist = true,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.removeTrack(1)

        assertThat(fakeInteractor.lastRemovedIndex).isEqualTo(1)
    }

    @Test
    fun `state maps track metadata correctly`() = runTest {
        val queueTrack = QueueScreenViewModel.Interactor.QueueTrack(
            trackId = "track-123",
            trackName = "Amazing Song",
            albumId = "album-456",
            artists = listOf(ArtistInfo("artist-1", "Artist One")),
            durationSeconds = 240,
        )
        fakeInteractor.queueStateFlow.value = QueueScreenViewModel.Interactor.QueueState(
            tracks = listOf(queueTrack),
            currentIndex = 0,
            contextName = "Test Album",
            contextType = QueueContextType.Album,
            canSaveAsPlaylist = false,
        )

        createViewModel()
        advanceUntilIdle()

        val track = viewModel.state.value.tracks.first()
        assertThat(track.trackId).isEqualTo("track-123")
        assertThat(track.trackName).isEqualTo("Amazing Song")
        assertThat(track.albumId).isEqualTo("album-456")
        assertThat(track.artists).hasSize(1)
        assertThat(track.artists.first().name).isEqualTo("Artist One")
        assertThat(track.durationSeconds).isEqualTo(240)
    }

    private fun createQueueTrack(trackId: String) = QueueScreenViewModel.Interactor.QueueTrack(
        trackId = trackId,
        trackName = "Track $trackId",
        albumId = "album-1",
        artists = emptyList(),
        durationSeconds = 180,
    )

    private class FakeInteractor : QueueScreenViewModel.Interactor {
        val queueStateFlow = MutableStateFlow<QueueScreenViewModel.Interactor.QueueState?>(null)
        val isRemoteFlow = MutableStateFlow(false)

        var lastPlayedIndex: Int? = null
        var lastMoveFrom: Int? = null
        var lastMoveTo: Int? = null
        var lastRemovedIndex: Int? = null

        override fun getQueueState(): Flow<QueueScreenViewModel.Interactor.QueueState?> = queueStateFlow
        override fun getIsRemote(): Flow<Boolean> = isRemoteFlow

        override fun playTrackAtIndex(index: Int) {
            lastPlayedIndex = index
        }

        override fun moveTrack(fromIndex: Int, toIndex: Int) {
            lastMoveFrom = fromIndex
            lastMoveTo = toIndex
        }

        override fun removeTrack(index: Int) {
            lastRemovedIndex = index
        }

        override fun playTrackDirectly(trackId: String) {
            // Not tested
        }

        override fun addTrackToQueue(trackId: String) {
            // Not tested
        }

        override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
            // Not tested
        }

        override suspend fun createPlaylist(name: String) {
            // Not tested
        }

        override fun toggleLike(trackId: String, currentlyLiked: Boolean) {
            // Not tested
        }

        override fun isLiked(trackId: String): Flow<Boolean> = flowOf(false)

        override fun getUserPlaylists(): Flow<List<UiUserPlaylist>> = flowOf(emptyList())
    }
}
