package com.lelloman.pezzottify.android.ui.screen.main.library

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.model.LikedContent
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
class LibraryScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var viewModel: LibraryScreenViewModel

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
        viewModel = LibraryScreenViewModel(
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
    fun `state shows liked albums from interactor`() = runTest {
        fakeInteractor.likedContentFlow.value = listOf(
            LikedContent("album-1", LikedContent.ContentType.Album, isLiked = true),
            LikedContent("album-2", LikedContent.ContentType.Album, isLiked = true),
            LikedContent("album-3", LikedContent.ContentType.Album, isLiked = false), // unliked
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.likedAlbumIds).containsExactly("album-1", "album-2")
    }

    @Test
    fun `state shows liked artists from interactor`() = runTest {
        fakeInteractor.likedContentFlow.value = listOf(
            LikedContent("artist-1", LikedContent.ContentType.Artist, isLiked = true),
            LikedContent("artist-2", LikedContent.ContentType.Artist, isLiked = true),
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.likedArtistIds).containsExactly("artist-1", "artist-2")
    }

    @Test
    fun `state shows liked tracks from interactor`() = runTest {
        fakeInteractor.likedContentFlow.value = listOf(
            LikedContent("track-1", LikedContent.ContentType.Track, isLiked = true),
            LikedContent("track-2", LikedContent.ContentType.Track, isLiked = true),
            LikedContent("track-3", LikedContent.ContentType.Track, isLiked = true),
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.likedTrackIds).containsExactly("track-1", "track-2", "track-3")
    }

    @Test
    fun `state shows playlists from interactor`() = runTest {
        val playlists = listOf(
            UiUserPlaylist("playlist-1", "My Playlist 1", trackCount = 5),
            UiUserPlaylist("playlist-2", "My Playlist 2", trackCount = 10),
        )
        fakeInteractor.playlistsFlow.value = playlists

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.playlists).isEqualTo(playlists)
    }

    @Test
    fun `state separates content by type correctly`() = runTest {
        fakeInteractor.likedContentFlow.value = listOf(
            LikedContent("album-1", LikedContent.ContentType.Album, isLiked = true),
            LikedContent("artist-1", LikedContent.ContentType.Artist, isLiked = true),
            LikedContent("track-1", LikedContent.ContentType.Track, isLiked = true),
            LikedContent("album-2", LikedContent.ContentType.Album, isLiked = true),
            LikedContent("artist-2", LikedContent.ContentType.Artist, isLiked = true),
        )

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.likedAlbumIds).containsExactly("album-1", "album-2")
        assertThat(viewModel.state.value.likedArtistIds).containsExactly("artist-1", "artist-2")
        assertThat(viewModel.state.value.likedTrackIds).containsExactly("track-1")
    }

    @Test
    fun `createPlaylist calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.createPlaylist("New Playlist")
        advanceUntilIdle()

        assertThat(fakeInteractor.lastCreatedPlaylistName).isEqualTo("New Playlist")
    }

    private class FakeInteractor : LibraryScreenViewModel.Interactor {
        val likedContentFlow = MutableStateFlow<List<LikedContent>>(emptyList())
        val playlistsFlow = MutableStateFlow<List<UiUserPlaylist>>(emptyList())

        var lastCreatedPlaylistName: String? = null

        override fun getLikedContent(): Flow<List<LikedContent>> = likedContentFlow

        override fun getPlaylists(): Flow<List<UiUserPlaylist>> = playlistsFlow

        override suspend fun createPlaylist(name: String) {
            lastCreatedPlaylistName = name
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
