package com.lelloman.pezzottify.android.ui.screen.queue

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylist
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
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
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var viewModel: QueueScreenViewModel

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
        viewModel = QueueScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state shows error when playlist is null`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = null

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isError).isTrue()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `state shows tracks from playlist`() = runTest {
        val playlist = PlaybackPlaylist(
            tracksIds = listOf("track-1", "track-2", "track-3"),
            context = PlaybackPlaylistContext.Album("album-1"),
        )
        fakeInteractor.playbackPlaylistFlow.value = playlist
        fakeInteractor.currentTrackIndexFlow.value = 1

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isError).isFalse()
        assertThat(viewModel.state.value.tracks).hasSize(3)
        assertThat(viewModel.state.value.currentTrackIndex).isEqualTo(1)
    }

    @Test
    fun `state shows Album context type for album playlist`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1"),
            context = PlaybackPlaylistContext.Album("album-1"),
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.Album)
        assertThat(viewModel.state.value.contextName).isEqualTo("album-1")
        assertThat(viewModel.state.value.canSaveAsPlaylist).isFalse()
    }

    @Test
    fun `state shows UserPlaylist context type for user playlist`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1"),
            context = PlaybackPlaylistContext.UserPlaylist("playlist-1", isEdited = true),
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.UserPlaylist)
        assertThat(viewModel.state.value.contextName).isEqualTo("playlist-1")
        assertThat(viewModel.state.value.canSaveAsPlaylist).isTrue()
    }

    @Test
    fun `state shows UserMix context type for user mix`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1"),
            context = PlaybackPlaylistContext.UserMix,
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.contextType).isEqualTo(QueueContextType.UserMix)
        assertThat(viewModel.state.value.canSaveAsPlaylist).isTrue()
    }

    @Test
    fun `clickOnTrack calls interactor playTrackAtIndex`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1", "track-2"),
            context = PlaybackPlaylistContext.UserMix,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.clickOnTrack(1)

        assertThat(fakeInteractor.lastPlayedIndex).isEqualTo(1)
    }

    @Test
    fun `moveTrack calls interactor moveTrack`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1", "track-2", "track-3"),
            context = PlaybackPlaylistContext.UserMix,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.moveTrack(0, 2)

        assertThat(fakeInteractor.lastMoveFrom).isEqualTo(0)
        assertThat(fakeInteractor.lastMoveTo).isEqualTo(2)
    }

    @Test
    fun `removeTrack calls interactor removeTrack`() = runTest {
        fakeInteractor.playbackPlaylistFlow.value = PlaybackPlaylist(
            tracksIds = listOf("track-1", "track-2"),
            context = PlaybackPlaylistContext.UserMix,
        )

        createViewModel()
        advanceUntilIdle()

        viewModel.removeTrack("track-1")

        assertThat(fakeInteractor.lastRemovedTrackId).isEqualTo("track-1")
    }

    private class FakeInteractor : QueueScreenViewModel.Interactor {
        val playbackPlaylistFlow = MutableStateFlow<PlaybackPlaylist?>(null)
        val currentTrackIndexFlow = MutableStateFlow<Int?>(null)

        var lastPlayedIndex: Int? = null
        var lastMoveFrom: Int? = null
        var lastMoveTo: Int? = null
        var lastRemovedTrackId: String? = null

        override fun getPlaybackPlaylist(): Flow<PlaybackPlaylist?> = playbackPlaylistFlow

        override fun getCurrentTrackIndex(): Flow<Int?> = currentTrackIndexFlow

        override fun playTrackAtIndex(index: Int) {
            lastPlayedIndex = index
        }

        override fun moveTrack(fromIndex: Int, toIndex: Int) {
            lastMoveFrom = fromIndex
            lastMoveTo = toIndex
        }

        override fun removeTrack(trackId: String) {
            lastRemovedTrackId = trackId
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
            flowOf(Content.Resolved(trackId, Track(trackId, "Track $trackId", "album-1", emptyList(), 180)))

        override fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>> =
            flowOf(Content.Loading(artistId))
    }
}
