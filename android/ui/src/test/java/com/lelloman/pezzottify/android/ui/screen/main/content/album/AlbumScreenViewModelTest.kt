package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.navigation.NavController
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Disc
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
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
class AlbumScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var navController: NavController
    private lateinit var viewModel: AlbumScreenViewModel

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

    private fun createViewModel(albumId: String = "album-1") {
        viewModel = AlbumScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            albumId = albumId,
            navController = navController,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state shows error when album fails to load`() = runTest {
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Error("album-1"))

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isError).isTrue()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `state shows album from content resolver`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Epic Album",
            date = 1609459200L,
            imageUrl = "http://img.com/cover.jpg",
            artistsIds = listOf("artist-1"),
            discs = listOf(Disc("Disc 1", listOf("track-1", "track-2")))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isError).isFalse()
        assertThat(viewModel.state.value.album).isEqualTo(album)
        assertThat(viewModel.state.value.album?.name).isEqualTo("Epic Album")
    }

    @Test
    fun `state shows tracks from album discs`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = listOf(Disc("Disc 1", listOf("track-1", "track-2", "track-3")))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.tracks).hasSize(3)
    }

    @Test
    fun `state shows current playing track id`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        fakeInteractor.currentPlayingTrackIdFlow.value = "track-2"
        advanceUntilIdle()

        assertThat(viewModel.state.value.currentPlayingTrackId).isEqualTo("track-2")
    }

    @Test
    fun `state shows liked status`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))
        fakeInteractor.likedContentIds.add("album-1")

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLiked).isTrue()
    }

    @Test
    fun `state shows user playlists`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))
        val playlists = listOf(
            UiUserPlaylist("pl-1", "My Playlist", trackCount = 5),
            UiUserPlaylist("pl-2", "Favorites", trackCount = 10),
        )
        fakeInteractor.userPlaylistsFlow.value = playlists

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.userPlaylists).isEqualTo(playlists)
    }

    @Test
    fun `clickOnPlayAlbum calls interactor playAlbum`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.clickOnPlayAlbum("album-1")

        assertThat(fakeInteractor.lastPlayedAlbumId).isEqualTo("album-1")
    }

    @Test
    fun `clickOnTrack calls interactor playTrack`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = listOf(Disc("Disc 1", listOf("track-1")))
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.clickOnTrack("track-1")

        assertThat(fakeInteractor.lastPlayedTrackAlbumId).isEqualTo("album-1")
        assertThat(fakeInteractor.lastPlayedTrackId).isEqualTo("track-1")
    }

    @Test
    fun `clickOnLike calls interactor toggleLike`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.clickOnLike()

        assertThat(fakeInteractor.lastToggleLikeContentId).isEqualTo("album-1")
        assertThat(fakeInteractor.lastToggleLikeCurrentlyLiked).isFalse()
    }

    @Test
    fun `addTrackToQueue calls interactor`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.addTrackToQueue("track-5")

        assertThat(fakeInteractor.lastAddedToQueueTrackId).isEqualTo("track-5")
    }

    @Test
    fun `addAlbumToQueue calls interactor`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.addAlbumToQueue("album-1")

        assertThat(fakeInteractor.lastAddedToQueueAlbumId).isEqualTo("album-1")
    }

    @Test
    fun `playTrackDirectly calls interactor`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.playTrackDirectly("track-3")

        assertThat(fakeInteractor.lastPlayTrackDirectlyId).isEqualTo("track-3")
    }

    @Test
    fun `logs viewed album when loaded`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.loggedViewedAlbumId).isEqualTo("album-1")
    }

    @Test
    fun `createPlaylist calls interactor`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.createPlaylist("New Playlist")
        advanceUntilIdle()

        assertThat(fakeInteractor.lastCreatedPlaylistName).isEqualTo("New Playlist")
    }

    @Test
    fun `addTrackToPlaylist calls interactor`() = runTest {
        val album = Album(
            id = "album-1",
            name = "Album",
            date = 1609459200L,
            imageUrl = null,
            artistsIds = emptyList(),
            discs = emptyList()
        )
        fakeContentResolver.albumResults["album-1"] = flowOf(Content.Resolved("album-1", album))

        createViewModel("album-1")
        advanceUntilIdle()

        viewModel.addTrackToPlaylist("track-1", "playlist-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.lastAddedTrackToPlaylistTrackId).isEqualTo("track-1")
        assertThat(fakeInteractor.lastAddedTrackToPlaylistPlaylistId).isEqualTo("playlist-1")
    }

    private class FakeInteractor : AlbumScreenViewModel.Interactor {
        val currentPlayingTrackIdFlow = MutableStateFlow<String?>(null)
        val likedContentIds = mutableSetOf<String>()
        val userPlaylistsFlow = MutableStateFlow<List<UiUserPlaylist>>(emptyList())

        var lastPlayedAlbumId: String? = null
        var lastPlayedTrackAlbumId: String? = null
        var lastPlayedTrackId: String? = null
        var loggedViewedAlbumId: String? = null
        var lastToggleLikeContentId: String? = null
        var lastToggleLikeCurrentlyLiked: Boolean? = null
        var lastPlayTrackDirectlyId: String? = null
        var lastAddedToQueueTrackId: String? = null
        var lastAddedToQueueAlbumId: String? = null
        var lastAddedTrackToPlaylistTrackId: String? = null
        var lastAddedTrackToPlaylistPlaylistId: String? = null
        var lastAddedAlbumToPlaylistAlbumId: String? = null
        var lastAddedAlbumToPlaylistPlaylistId: String? = null
        var lastCreatedPlaylistName: String? = null

        override fun playAlbum(albumId: String) {
            lastPlayedAlbumId = albumId
        }

        override fun playTrack(albumId: String, trackId: String) {
            lastPlayedTrackAlbumId = albumId
            lastPlayedTrackId = trackId
        }

        override fun logViewedAlbum(albumId: String) {
            loggedViewedAlbumId = albumId
        }

        override fun getCurrentPlayingTrackId(): Flow<String?> = currentPlayingTrackIdFlow

        override fun isLiked(contentId: String): Flow<Boolean> =
            MutableStateFlow(likedContentIds.contains(contentId))

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            lastToggleLikeContentId = contentId
            lastToggleLikeCurrentlyLiked = currentlyLiked
        }

        override fun getUserPlaylists(): Flow<List<UiUserPlaylist>> = userPlaylistsFlow

        override fun playTrackDirectly(trackId: String) {
            lastPlayTrackDirectlyId = trackId
        }

        override fun addTrackToQueue(trackId: String) {
            lastAddedToQueueTrackId = trackId
        }

        override fun addAlbumToQueue(albumId: String) {
            lastAddedToQueueAlbumId = albumId
        }

        override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
            lastAddedTrackToPlaylistTrackId = trackId
            lastAddedTrackToPlaylistPlaylistId = playlistId
        }

        override suspend fun addAlbumToPlaylist(albumId: String, playlistId: String) {
            lastAddedAlbumToPlaylistAlbumId = albumId
            lastAddedAlbumToPlaylistPlaylistId = playlistId
        }

        override suspend fun createPlaylist(name: String) {
            lastCreatedPlaylistName = name
        }
    }

    private class FakeContentResolver : ContentResolver {
        val albumResults = mutableMapOf<String, Flow<Content<Album>>>()
        val trackResults = mutableMapOf<String, Flow<Content<Track>>>()

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
