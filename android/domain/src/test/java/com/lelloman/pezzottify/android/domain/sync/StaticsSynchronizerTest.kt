package com.lelloman.pezzottify.android.domain.sync

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumData
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumType
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistData
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackData
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.statics.StaticItemType
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
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
class StaticsSynchronizerTest {

    private lateinit var fetchStateStore: StaticItemFetchStateStore
    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var staticsStore: StaticsStore
    private lateinit var timeProvider: TimeProvider
    private lateinit var loggerFactory: LoggerFactory

    private var currentTime = 1_000_000L

    private val testDispatcher = UnconfinedTestDispatcher()
    private lateinit var testScope: CoroutineScope

    private lateinit var synchronizer: StaticsSynchronizer

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        testScope = CoroutineScope(testDispatcher)

        fetchStateStore = mockk(relaxed = true)
        remoteApiClient = mockk(relaxed = true)
        staticsStore = mockk(relaxed = true)
        timeProvider = TimeProvider { currentTime }

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getLogger(any<kotlin.reflect.KClass<*>>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        // Default behavior: no items to fetch, no loading items
        coEvery { fetchStateStore.getIdle() } returns emptyList()
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.resetLoadingStates() } returns Result.success(Unit)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)
        coEvery { fetchStateStore.delete(any()) } returns Result.success(Unit)

        coEvery { staticsStore.storeArtist(any()) } returns Result.success(Unit)
        coEvery { staticsStore.storeAlbum(any()) } returns Result.success(Unit)
        coEvery { staticsStore.storeTrack(any()) } returns Result.success(Unit)
        coEvery { staticsStore.storeDiscography(any()) } returns Result.success(Unit)
    }

    @After
    fun tearDown() {
        testScope.cancel()
        Dispatchers.resetMain()
    }

    private fun createSynchronizer(): StaticsSynchronizer {
        return StaticsSynchronizer(
            fetchStateStore = fetchStateStore,
            remoteApiClient = remoteApiClient,
            staticsStore = staticsStore,
            timeProvider = timeProvider,
            loggerFactory = loggerFactory,
            dispatcher = testDispatcher,
            scope = testScope,
        )
    }

    // ========== Initialization Tests ==========

    @Test
    fun `initialize resets loading states at startup`() = runTest {
        synchronizer = createSynchronizer()

        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { fetchStateStore.resetLoadingStates() }
    }

    @Test
    fun `initialize can only be called once`() = runTest {
        synchronizer = createSynchronizer()

        synchronizer.initialize()
        advanceUntilIdle()

        synchronizer.initialize()
        advanceUntilIdle()

        // resetLoadingStates should only be called once
        coVerify(exactly = 1) { fetchStateStore.resetLoadingStates() }
    }

    // ========== Main Loop Sleep/Wake Tests ==========

    @Test
    fun `main loop goes to sleep when no items to fetch and no loading`() = runTest {
        var getIdleCallCount = 0
        coEvery { fetchStateStore.getIdle() } answers {
            getIdleCallCount++
            emptyList()
        }
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // Should have called getIdle once and then gone to sleep
        assertThat(getIdleCallCount).isEqualTo(1)
    }

    @Test
    fun `main loop wakes up when wakeUp is called`() = runTest {
        var callCount = 0
        coEvery { fetchStateStore.getIdle() } answers {
            callCount++
            emptyList()
        }
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        assertThat(callCount).isEqualTo(1)

        // Wake up the synchronizer
        synchronizer.wakeUp()
        advanceUntilIdle()

        // Should have called getIdle again after waking up
        assertThat(callCount).isEqualTo(2)
    }

    @Test
    fun `main loop continues iterating when there are loading items`() = runTest {
        var iterationCount = 0
        coEvery { fetchStateStore.getIdle() } answers {
            iterationCount++
            emptyList()
        }
        // First return 1 loading item, then 0 to let the loop sleep
        coEvery { fetchStateStore.getLoadingItemsCount() } returnsMany listOf(1, 1, 0)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceTimeBy(20_000) // Allow time for multiple iterations
        advanceUntilIdle()

        // Should have iterated multiple times due to loading items
        assertThat(iterationCount).isAtLeast(2)
    }

    // ========== Fetch Success Tests ==========

    @Test
    fun `successful artist fetch stores data and deletes fetch state`() = runTest {
        val artistId = "artist-123"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val artistResponse = createArtistResponse(artistId)

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Success(artistResponse)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.getArtist(artistId) }
        coVerify { staticsStore.storeArtist(any()) }
        coVerify { fetchStateStore.delete(artistId) }
    }

    @Test
    fun `successful album fetch stores data and deletes fetch state`() = runTest {
        val albumId = "album-456"
        val fetchState = createIdleFetchState(albumId, StaticItemType.Album)
        val albumResponse = createAlbumResponse(albumId)

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getAlbum(albumId) } returns RemoteApiResponse.Success(albumResponse)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.getAlbum(albumId) }
        coVerify { staticsStore.storeAlbum(any()) }
        coVerify { fetchStateStore.delete(albumId) }
    }

    @Test
    fun `successful track fetch stores data and deletes fetch state`() = runTest {
        val trackId = "track-789"
        val fetchState = createIdleFetchState(trackId, StaticItemType.Track)
        val trackResponse = createTrackResponse(trackId)

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getTrack(trackId) } returns RemoteApiResponse.Success(trackResponse)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.getTrack(trackId) }
        coVerify { staticsStore.storeTrack(any()) }
        coVerify { fetchStateStore.delete(trackId) }
    }

    @Test
    fun `successful discography fetch stores data and deletes fetch state`() = runTest {
        val artistId = "artist-discog"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Discography)
        val discographyResponse = createDiscographyResponse()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtistDiscography(artistId) } returns RemoteApiResponse.Success(discographyResponse)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.getArtistDiscography(artistId) }
        coVerify { staticsStore.storeDiscography(any()) }
        coVerify { fetchStateStore.delete(artistId) }
    }

    @Test
    fun `fetch sets loading state before making API call`() = runTest {
        val artistId = "artist-loading"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Success(createArtistResponse(artistId))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // First stored state should be loading
        assertThat(storedStates).isNotEmpty()
        val loadingState = storedStates.first()
        assertThat(loadingState.isLoading).isTrue()
        assertThat(loadingState.itemId).isEqualTo(artistId)
        assertThat(loadingState.itemType).isEqualTo(StaticItemType.Artist)
    }

    // ========== Error Handling Tests ==========

    @Test
    fun `network error creates error state with 1 minute retry delay`() = runTest {
        val artistId = "artist-network-error"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Error.Network

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // Find the error state (not the loading state)
        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.errorReason).isEqualTo(ErrorReason.Network)
        assertThat(errorState.tryNextTime).isEqualTo(currentTime + 60_000L) // 1 minute
    }

    @Test
    fun `unauthorized error creates error state with 30 minute retry delay`() = runTest {
        val artistId = "artist-unauthorized"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Error.Unauthorized

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.errorReason).isEqualTo(ErrorReason.Client)
        assertThat(errorState.tryNextTime).isEqualTo(currentTime + 1_800_000L) // 30 minutes
    }

    @Test
    fun `not found error creates error state with 1 hour retry delay`() = runTest {
        val artistId = "artist-not-found"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Error.NotFound

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.errorReason).isEqualTo(ErrorReason.NotFound)
        assertThat(errorState.tryNextTime).isEqualTo(currentTime + 3_600_000L) // 1 hour
    }

    @Test
    fun `unknown error creates error state with 5 minute retry delay`() = runTest {
        val artistId = "artist-unknown-error"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Error.Unknown("Something went wrong")

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.errorReason).isEqualTo(ErrorReason.Unknown)
        assertThat(errorState.tryNextTime).isEqualTo(currentTime + 300_000L) // 5 minutes
    }

    @Test
    fun `storage exception creates client error state with 5 minute retry delay`() = runTest {
        val artistId = "artist-storage-error"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Success(createArtistResponse(artistId))
        coEvery { staticsStore.storeArtist(any()) } throws RuntimeException("Storage failed")

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.errorReason).isEqualTo(ErrorReason.Client)
        assertThat(errorState.tryNextTime).isEqualTo(currentTime + 300_000L) // 5 minutes
    }

    // ========== Multiple Items Tests ==========

    @Test
    fun `processes multiple idle items in single iteration`() = runTest {
        val items = listOf(
            createIdleFetchState("artist-1", StaticItemType.Artist),
            createIdleFetchState("album-1", StaticItemType.Album),
            createIdleFetchState("track-1", StaticItemType.Track),
        )

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(items, emptyList())
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtist("artist-1") } returns RemoteApiResponse.Success(createArtistResponse("artist-1"))
        coEvery { remoteApiClient.getAlbum("album-1") } returns RemoteApiResponse.Success(createAlbumResponse("album-1"))
        coEvery { remoteApiClient.getTrack("track-1") } returns RemoteApiResponse.Success(createTrackResponse("track-1"))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.getArtist("artist-1") }
        coVerify { remoteApiClient.getAlbum("album-1") }
        coVerify { remoteApiClient.getTrack("track-1") }
        coVerify { fetchStateStore.delete("artist-1") }
        coVerify { fetchStateStore.delete("album-1") }
        coVerify { fetchStateStore.delete("track-1") }
    }

    @Test
    fun `error in one item does not prevent processing of other items`() = runTest {
        val items = listOf(
            createIdleFetchState("artist-fail", StaticItemType.Artist),
            createIdleFetchState("artist-success", StaticItemType.Artist),
        )

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(items, emptyList())
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtist("artist-fail") } returns RemoteApiResponse.Error.Network
        coEvery { remoteApiClient.getArtist("artist-success") } returns RemoteApiResponse.Success(createArtistResponse("artist-success"))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // Both should be processed
        coVerify { remoteApiClient.getArtist("artist-fail") }
        coVerify { remoteApiClient.getArtist("artist-success") }

        // Success should be deleted, failure should have error state stored
        coVerify { fetchStateStore.delete("artist-success") }
        coVerify(exactly = 0) { fetchStateStore.delete("artist-fail") }
    }

    // ========== State Transition Tests ==========

    @Test
    fun `loading state includes correct lastAttemptTime`() = runTest {
        val artistId = "artist-time"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        currentTime = 5_000_000L

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Success(createArtistResponse(artistId))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val loadingState = storedStates.first { it.isLoading }
        assertThat(loadingState.lastAttemptTime).isEqualTo(5_000_000L)
    }

    @Test
    fun `error state includes correct lastAttemptTime`() = runTest {
        val artistId = "artist-error-time"
        val fetchState = createIdleFetchState(artistId, StaticItemType.Artist)
        val storedStates = mutableListOf<StaticItemFetchState>()

        currentTime = 7_000_000L

        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(fetchState),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { fetchStateStore.store(any()) } answers {
            storedStates.add(firstArg())
            Result.success(Unit)
        }
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Error.Network

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        val errorState = storedStates.find { !it.isLoading && it.errorReason != null }
        assertThat(errorState).isNotNull()
        assertThat(errorState!!.lastAttemptTime).isEqualTo(7_000_000L)
    }

    // ========== API Dispatch Tests ==========

    @Test
    fun `Artist item type calls getArtist API`() = runTest {
        val artistId = "dispatch-artist"
        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(createIdleFetchState(artistId, StaticItemType.Artist)),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtist(artistId) } returns RemoteApiResponse.Success(createArtistResponse(artistId))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 1) { remoteApiClient.getArtist(artistId) }
        coVerify(exactly = 0) { remoteApiClient.getAlbum(any()) }
        coVerify(exactly = 0) { remoteApiClient.getTrack(any()) }
        coVerify(exactly = 0) { remoteApiClient.getArtistDiscography(any()) }
    }

    @Test
    fun `Album item type calls getAlbum API`() = runTest {
        val albumId = "dispatch-album"
        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(createIdleFetchState(albumId, StaticItemType.Album)),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getAlbum(albumId) } returns RemoteApiResponse.Success(createAlbumResponse(albumId))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 0) { remoteApiClient.getArtist(any()) }
        coVerify(exactly = 1) { remoteApiClient.getAlbum(albumId) }
        coVerify(exactly = 0) { remoteApiClient.getTrack(any()) }
        coVerify(exactly = 0) { remoteApiClient.getArtistDiscography(any()) }
    }

    @Test
    fun `Track item type calls getTrack API`() = runTest {
        val trackId = "dispatch-track"
        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(createIdleFetchState(trackId, StaticItemType.Track)),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getTrack(trackId) } returns RemoteApiResponse.Success(createTrackResponse(trackId))

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 0) { remoteApiClient.getArtist(any()) }
        coVerify(exactly = 0) { remoteApiClient.getAlbum(any()) }
        coVerify(exactly = 1) { remoteApiClient.getTrack(trackId) }
        coVerify(exactly = 0) { remoteApiClient.getArtistDiscography(any()) }
    }

    @Test
    fun `Discography item type calls getArtistDiscography API`() = runTest {
        val artistId = "dispatch-discog"
        coEvery { fetchStateStore.getIdle() } returnsMany listOf(
            listOf(createIdleFetchState(artistId, StaticItemType.Discography)),
            emptyList()
        )
        coEvery { fetchStateStore.getLoadingItemsCount() } returns 0
        coEvery { remoteApiClient.getArtistDiscography(artistId) } returns RemoteApiResponse.Success(createDiscographyResponse())

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 0) { remoteApiClient.getArtist(any()) }
        coVerify(exactly = 0) { remoteApiClient.getAlbum(any()) }
        coVerify(exactly = 0) { remoteApiClient.getTrack(any()) }
        coVerify(exactly = 1) { remoteApiClient.getArtistDiscography(artistId) }
    }

    // ========== Helper Functions ==========

    private fun createIdleFetchState(itemId: String, itemType: StaticItemType): StaticItemFetchState {
        return StaticItemFetchState.requested(itemId, itemType)
    }

    private fun createArtistResponse(artistId: String): ArtistResponse {
        return ArtistResponse(
            artist = ArtistData(
                id = artistId,
                name = "Test Artist",
                genres = listOf("Rock"),
                activityPeriods = emptyList(),
            ),
            displayImage = null,
            relatedArtists = emptyList(),
        )
    }

    private fun createAlbumResponse(albumId: String): AlbumResponse {
        return AlbumResponse(
            album = AlbumData(
                id = albumId,
                name = "Test Album",
                albumType = AlbumType.Album,
                label = null,
                date = null,
                genres = emptyList(),
                originalTitle = null,
                versionTitle = null,
            ),
            artists = emptyList(),
            discs = emptyList(),
            displayImage = null,
        )
    }

    private fun createTrackResponse(trackId: String): TrackResponse {
        return TrackResponse(
            track = TrackData(
                id = trackId,
                name = "Test Track",
                albumId = "album-1",
                discNumber = 1,
                trackNumber = 1,
                durationSecs = 180,
                isExplicit = false,
                audioUri = "https://example.com/audio",
                format = "mp3",
                tags = emptyList(),
                hasLyrics = false,
                languages = emptyList(),
                originalTitle = null,
                versionTitle = null,
            ),
            album = AlbumData(
                id = "album-1",
                name = "Test Album",
                albumType = AlbumType.Album,
                label = null,
                date = null,
                genres = emptyList(),
                originalTitle = null,
                versionTitle = null,
            ),
            artists = emptyList(),
        )
    }

    private fun createDiscographyResponse(): ArtistDiscographyResponse {
        return ArtistDiscographyResponse(
            albums = listOf(
                AlbumData(
                    id = "album-1",
                    name = "Album 1",
                    albumType = AlbumType.Album,
                    label = null,
                    date = null,
                    genres = emptyList(),
                    originalTitle = null,
                    versionTitle = null,
                )
            ),
            features = emptyList(),
        )
    }
}
