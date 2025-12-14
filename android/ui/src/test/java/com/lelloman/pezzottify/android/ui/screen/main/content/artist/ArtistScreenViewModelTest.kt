package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import android.util.Log
import androidx.navigation.NavController
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
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
class ArtistScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var fakeContentResolver: FakeContentResolver
    private lateinit var navController: NavController
    private lateinit var viewModel: ArtistScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        fakeContentResolver = FakeContentResolver()
        navController = mockk(relaxed = true)
        mockkStatic(Log::class)
        every { Log.d(any(), any()) } returns 0
        every { Log.e(any(), any(), any()) } returns 0
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
        unmockkStatic(Log::class)
    }

    private fun createViewModel(artistId: String = "artist-1") {
        viewModel = ArtistScreenViewModel(
            interactor = fakeInteractor,
            contentResolver = fakeContentResolver,
            artistId = artistId,
            navController = navController,
        )
    }

    @Test
    fun `initial state is loading`() = runTest {
        createViewModel()

        assertThat(viewModel.state.value.isLoading).isTrue()
    }

    @Test
    fun `state shows error when artist fails to load`() = runTest {
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Error("artist-1"))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isError).isTrue()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `state shows artist from content resolver`() = runTest {
        val artist = Artist(
            id = "artist-1",
            name = "Great Artist",
            imageUrl = "http://img.com/artist.jpg",
            related = listOf("artist-2", "artist-3")
        )
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(viewModel.state.value.isError).isFalse()
        assertThat(viewModel.state.value.artist).isEqualTo(artist)
        assertThat(viewModel.state.value.artist?.name).isEqualTo("Great Artist")
        assertThat(viewModel.state.value.relatedArtists).containsExactly("artist-2", "artist-3")
    }

    @Test
    fun `state shows discography from content resolver`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        val discography = ArtistDiscography(
            albums = listOf("album-1", "album-2"),
            features = listOf("album-3")
        )
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Resolved("artist-1", discography))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.albums).containsExactly("album-1", "album-2")
        assertThat(viewModel.state.value.features).containsExactly("album-3")
    }

    @Test
    fun `state shows liked status`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.likedContentIds.add("artist-1")

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLiked).isTrue()
    }

    @Test
    fun `clickOnLike calls interactor toggleLike`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))

        createViewModel("artist-1")
        advanceUntilIdle()

        viewModel.clickOnLike()

        assertThat(fakeInteractor.lastToggleLikeContentId).isEqualTo("artist-1")
        assertThat(fakeInteractor.lastToggleLikeCurrentlyLiked).isFalse()
    }

    @Test
    fun `logs viewed artist when loaded`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.loggedViewedArtistId).isEqualTo("artist-1")
    }

    @Test
    fun `does not log viewed artist when loading`() = runTest {
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(fakeInteractor.loggedViewedArtistId).isNull()
    }

    @Test
    fun `loads external albums when enabled`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = true
        fakeInteractor.externalDiscographyResult = Result.success(
            listOf(
                UiExternalAlbumItem("ext-1", "External Album 1", null, 2023, false),
                UiExternalAlbumItem("ext-2", "External Album 2", null, 2022, false),
            )
        )

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.externalAlbums).hasSize(2)
        assertThat(viewModel.state.value.externalAlbums[0].name).isEqualTo("External Album 1")
    }

    @Test
    fun `does not load external albums when disabled`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = false

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.externalAlbums).isEmpty()
        assertThat(viewModel.state.value.isExternalAlbumsError).isFalse()
    }

    @Test
    fun `handles external album load failure gracefully`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = true
        fakeInteractor.externalDiscographyResult = Result.failure(RuntimeException("Network error"))

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.externalAlbums).isEmpty()
        assertThat(viewModel.state.value.isExternalAlbumsError).isTrue()
    }

    @Test
    fun `hasLoadedExternalAlbums is true after successful load`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = true
        fakeInteractor.externalDiscographyResult = Result.success(
            listOf(UiExternalAlbumItem("ext-1", "Album", null, 2023, false))
        )

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasLoadedExternalAlbums).isTrue()
    }

    @Test
    fun `hasLoadedExternalAlbums is true even with empty result`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = true
        fakeInteractor.externalDiscographyResult = Result.success(emptyList())

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasLoadedExternalAlbums).isTrue()
        assertThat(viewModel.state.value.externalAlbums).isEmpty()
        assertThat(viewModel.state.value.isExternalAlbumsError).isFalse()
    }

    @Test
    fun `hasLoadedExternalAlbums is false when external albums disabled`() = runTest {
        val artist = Artist("artist-1", "Artist", null, emptyList())
        fakeContentResolver.artistResults["artist-1"] = flowOf(Content.Resolved("artist-1", artist))
        fakeContentResolver.discographyResults["artist-1"] = flowOf(Content.Loading("artist-1"))
        fakeInteractor.canShowExternalAlbumsResult = false

        createViewModel("artist-1")
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasLoadedExternalAlbums).isFalse()
    }

    private class FakeInteractor : ArtistScreenViewModel.Interactor {
        val likedContentIds = mutableSetOf<String>()

        var loggedViewedArtistId: String? = null
        var lastToggleLikeContentId: String? = null
        var lastToggleLikeCurrentlyLiked: Boolean? = null
        var canShowExternalAlbumsResult = false
        var externalDiscographyResult: Result<List<UiExternalAlbumItem>> = Result.success(emptyList())

        override fun logViewedArtist(artistId: String) {
            loggedViewedArtistId = artistId
        }

        override fun isLiked(contentId: String): Flow<Boolean> =
            MutableStateFlow(likedContentIds.contains(contentId))

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            lastToggleLikeContentId = contentId
            lastToggleLikeCurrentlyLiked = currentlyLiked
        }

        override suspend fun canShowExternalAlbums(): Boolean = canShowExternalAlbumsResult

        override suspend fun getExternalDiscography(artistId: String): Result<List<UiExternalAlbumItem>> =
            externalDiscographyResult
    }

    private class FakeContentResolver : ContentResolver {
        val artistResults = mutableMapOf<String, Flow<Content<Artist>>>()
        val discographyResults = mutableMapOf<String, Flow<Content<ArtistDiscography>>>()

        override fun resolveSearchResult(
            itemId: String,
            itemType: SearchScreenViewModel.SearchedItemType
        ): Flow<Content<SearchResultContent>> = flowOf(Content.Loading(itemId))

        override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
            artistResults[artistId] ?: flowOf(Content.Loading(artistId))

        override fun resolveAlbum(albumId: String): Flow<Content<Album>> =
            flowOf(Content.Loading(albumId))

        override fun resolveTrack(trackId: String): Flow<Content<Track>> =
            flowOf(Content.Loading(trackId))

        override fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>> =
            discographyResults[artistId] ?: flowOf(Content.Loading(artistId))
    }
}
