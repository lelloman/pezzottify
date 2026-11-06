package com.lelloman.pezzottify.android.domain.player.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.QueueLoadingState
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackMetadataProviderImplTest {

    private val dispatcher = StandardTestDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private val playlist = MutableStateFlow<PlaybackPlaylist?>(null)
    private val currentTrackIndex = MutableStateFlow<Int?>(null)
    private val player = mockk<PezzottifyPlayer>(relaxed = true) {
        every { playbackPlaylist } returns playlist
    }
    private val platformPlayer = mockk<PlatformPlayer>(relaxed = true) {
        every { currentTrackIndex } returns this@PlaybackMetadataProviderImplTest.currentTrackIndex
    }
    private val configStore = mockk<ConfigStore> {
        every { baseUrl } returns MutableStateFlow("http://server/")
    }
    private val staticsProvider = mockk<StaticsProvider>()

    private val trackFlows = mutableMapOf<String, MutableStateFlow<StaticsItem<out Track>>>()
    private val albumFlows = mutableMapOf<String, MutableStateFlow<StaticsItem<out Album>>>()
    private val artistFlows = mutableMapOf<String, MutableStateFlow<StaticsItem<out Artist>>>()

    private lateinit var provider: PlaybackMetadataProviderImpl

    @Before
    fun setUp() {
        every { staticsProvider.provideTrack(any()) } answers {
            val id = firstArg<String>()
            trackFlows.getOrPut(id) { MutableStateFlow(StaticsItem.Loading(id)) }
        }
        every { staticsProvider.provideAlbum(any()) } answers {
            val id = firstArg<String>()
            albumFlows.getOrPut(id) { MutableStateFlow(StaticsItem.Loading(id)) }
        }
        every { staticsProvider.provideArtist(any()) } answers {
            val id = firstArg<String>()
            artistFlows.getOrPut(id) { MutableStateFlow(StaticsItem.Loading(id)) }
        }

        val logger = mockk<Logger>(relaxed = true)
        val loggerFactory = mockk<LoggerFactory> {
            every { getLogger(any<String>()) } returns logger
            every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns logger
            every { getValue(any(), any()) } returns logger
        }

        provider = PlaybackMetadataProviderImpl(
            player = player,
            platformPlayer = platformPlayer,
            staticsProvider = staticsProvider,
            configStore = configStore,
            loggerFactory = loggerFactory,
            scope = scope,
        )
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    private fun track(id: String, albumId: String = "album-$id", artistIds: List<String> = listOf("artist-$id")) =
        mockk<Track> {
            every { this@mockk.id } returns id
            every { name } returns "Track $id"
            every { this@mockk.albumId } returns albumId
            every { artistsIds } returns artistIds
            every { durationSeconds } returns 180
            every { availability } returns TrackAvailability.Available
        }

    private fun album(id: String) = mockk<Album> {
        every { this@mockk.id } returns id
        every { name } returns "Album $id"
        every { displayImageId } returns "img-$id"
    }

    private fun artist(id: String) = mockk<Artist> {
        every { this@mockk.id } returns id
        every { name } returns "Artist $id"
    }

    private fun loadTrack(id: String) {
        trackFlows.getValue(id).value = StaticsItem.Loaded(id, track(id))
    }

    // Creates the flows if the provider has not asked for them yet (same object it will get).
    private fun loadAlbumAndArtist(trackId: String) {
        val albumId = "album-$trackId"
        val artistId = "artist-$trackId"
        albumFlows.getOrPut(albumId) { MutableStateFlow(StaticsItem.Loading(albumId)) }.value =
            StaticsItem.Loaded(albumId, album(albumId))
        artistFlows.getOrPut(artistId) { MutableStateFlow(StaticsItem.Loading(artistId)) }.value =
            StaticsItem.Loaded(artistId, artist(artistId))
    }

    private fun setPlaylist(vararg ids: String) {
        playlist.value = PlaybackPlaylist(PlaybackPlaylistContext.UserMix, ids.toList())
    }

    @Test
    fun `emits loading then loaded once everything resolves within the first pass`() = runTest(dispatcher) {
        setPlaylist("t1")
        runCurrent()
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)
        assertThat(provider.queueState.value?.tracks).isEmpty()

        loadTrack("t1")
        runCurrent()
        // Album and artist still pending: still in the first pass.
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)

        loadAlbumAndArtist("t1")
        runCurrent()
        val state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        val metadata = state.tracks.single()
        assertThat(metadata.trackName).isEqualTo("Track t1")
        assertThat(metadata.artistNames).containsExactly("Artist artist-t1")
        assertThat(metadata.albumName).isEqualTo("Album album-t1")
        assertThat(metadata.artworkUrl).isEqualTo("http://server/v1/content/image/img-album-t1")
        assertThat(metadata.durationSeconds).isEqualTo(180)
    }

    @Test
    fun `cache hits resolve without waiting for the timeout`() = runTest(dispatcher) {
        every { staticsProvider.provideTrack("t1") } returns flowOf(StaticsItem.Loaded("t1", track("t1")))
        every { staticsProvider.provideAlbum("album-t1") } returns
            flowOf(StaticsItem.Loaded("album-t1", album("album-t1")))
        every { staticsProvider.provideArtist("artist-t1") } returns
            flowOf(StaticsItem.Loaded("artist-t1", artist("artist-t1")))

        setPlaylist("t1")
        runCurrent()

        val state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        assertThat(state.tracks.single().trackName).isEqualTo("Track t1")
    }

    @Test
    fun `track loaded after the first pass patches the queue state`() = runTest(dispatcher) {
        setPlaylist("t1", "t2")
        runCurrent()
        loadTrack("t1")
        loadAlbumAndArtist("t1")
        runCurrent()
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)

        // First pass times out with t2 still missing: placeholder for t2.
        advanceTimeBy(QueueMetadataResolver.DEFAULT_FIRST_PASS_TIMEOUT.inWholeMilliseconds + 1)
        runCurrent()
        var state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        assertThat(state.tracks.map { it.trackName }).containsExactly("Track t1", "t2").inOrder()
        assertThat(state.tracks[1].artistNames).isEmpty()
        assertThat(state.tracks[1].durationSeconds).isEqualTo(0)

        // t2 lands later: track fields first, then album/artist.
        loadTrack("t2")
        runCurrent()
        state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        assertThat(state.tracks[1].trackName).isEqualTo("Track t2")
        assertThat(state.tracks[1].durationSeconds).isEqualTo(180)
        assertThat(state.tracks[1].albumName).isEmpty()

        loadAlbumAndArtist("t2")
        runCurrent()
        val t2 = provider.queueState.value!!.tracks[1]
        assertThat(t2.artistNames).containsExactly("Artist artist-t2")
        assertThat(t2.albumName).isEqualTo("Album album-t2")
        assertThat(t2.artworkUrl).isEqualTo("http://server/v1/content/image/img-album-t2")
        assertThat(t2.imageId).isEqualTo("img-album-t2")
    }

    @Test
    fun `playlist change cancels the previous resolution and replaces the queue`() = runTest(dispatcher) {
        setPlaylist("t1")
        runCurrent()
        advanceTimeBy(QueueMetadataResolver.DEFAULT_FIRST_PASS_TIMEOUT.inWholeMilliseconds + 1)
        runCurrent()
        assertThat(provider.queueState.value!!.tracks.map { it.trackId }).containsExactly("t1")

        setPlaylist("t3")
        runCurrent()
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)
        assertThat(trackFlows.getValue("t1").subscriptionCount.value).isEqualTo(0)

        // A late load for the old playlist must not leak into the new queue.
        loadTrack("t1")
        runCurrent()
        assertThat(provider.queueState.value?.loadingState).isEqualTo(QueueLoadingState.LOADING)

        loadTrack("t3")
        loadAlbumAndArtist("t3")
        runCurrent()
        val state = provider.queueState.value!!
        assertThat(state.loadingState).isEqualTo(QueueLoadingState.LOADED)
        assertThat(state.tracks.map { it.trackName }).containsExactly("Track t3")
    }

    @Test
    fun `loaded items stop being observed`() = runTest(dispatcher) {
        setPlaylist("t1")
        runCurrent()
        assertThat(trackFlows.getValue("t1").subscriptionCount.value).isEqualTo(1)
        loadTrack("t1")
        loadAlbumAndArtist("t1")
        runCurrent()
        assertThat(trackFlows.getValue("t1").subscriptionCount.value).isEqualTo(0)
        assertThat(albumFlows.getValue("album-t1").subscriptionCount.value).isEqualTo(0)
        assertThat(artistFlows.getValue("artist-t1").subscriptionCount.value).isEqualTo(0)
    }

    @Test
    fun `current index follows the platform player across partial updates`() = runTest(dispatcher) {
        currentTrackIndex.value = 1
        setPlaylist("t1", "t2")
        runCurrent()
        assertThat(provider.queueState.value?.currentIndex).isEqualTo(1)

        advanceTimeBy(QueueMetadataResolver.DEFAULT_FIRST_PASS_TIMEOUT.inWholeMilliseconds + 1)
        runCurrent()
        assertThat(provider.queueState.value?.currentIndex).isEqualTo(1)

        currentTrackIndex.value = 0
        runCurrent()
        assertThat(provider.queueState.value?.currentIndex).isEqualTo(0)

        loadTrack("t2")
        runCurrent()
        val state = provider.queueState.value!!
        assertThat(state.currentIndex).isEqualTo(0)
        assertThat(state.tracks[1].trackName).isEqualTo("Track t2")
        assertThat(state.currentTrack?.trackId).isEqualTo("t1")
    }

    @Test
    fun `clearing the playlist resets the state and stops resolving`() = runTest(dispatcher) {
        setPlaylist("t1")
        runCurrent()
        playlist.value = null
        runCurrent()
        assertThat(provider.queueState.value).isNull()
        loadTrack("t1")
        advanceUntilIdle()
        assertThat(provider.queueState.value).isNull()
    }
}
