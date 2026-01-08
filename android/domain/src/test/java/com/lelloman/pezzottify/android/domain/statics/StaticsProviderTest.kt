package com.lelloman.pezzottify.android.domain.statics

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollector
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.skeleton.DiscographyCacheFetcher
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.statics.StaticsSynchronizer
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class StaticsProviderTest {

    private lateinit var staticsStore: StaticsStore
    private lateinit var fetchStateStore: StaticItemFetchStateStore
    private lateinit var staticsSynchronizer: StaticsSynchronizer
    private lateinit var timeProvider: TimeProvider
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var staticsCache: StaticsCache
    private lateinit var cacheMetricsCollector: CacheMetricsCollector
    private lateinit var userSettingsStore: UserSettingsStore
    private lateinit var skeletonStore: SkeletonStore
    private lateinit var discographyCacheFetcher: DiscographyCacheFetcher

    private var currentTime = 1_000_000L

    private lateinit var staticsProvider: StaticsProvider

    @Before
    fun setUp() {
        staticsStore = mockk()
        fetchStateStore = mockk()
        staticsSynchronizer = mockk(relaxed = true)
        timeProvider = TimeProvider { currentTime }
        staticsCache = mockk(relaxed = true)
        cacheMetricsCollector = mockk(relaxed = true)
        userSettingsStore = mockk()
        skeletonStore = mockk(relaxed = true)
        discographyCacheFetcher = mockk(relaxed = true)

        // Disable cache for these tests (testing Room flow behavior)
        every { userSettingsStore.isInMemoryCacheEnabled } returns MutableStateFlow(false)

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        staticsProvider = StaticsProvider(
            staticsStore = staticsStore,
            staticItemFetchStateStore = fetchStateStore,
            staticsSynchronizer = staticsSynchronizer,
            timeProvider = timeProvider,
            staticsCache = staticsCache,
            cacheMetricsCollector = cacheMetricsCollector,
            userSettingsStore = userSettingsStore,
            skeletonStore = skeletonStore,
            discographyCacheFetcher = discographyCacheFetcher,
            loggerFactory = loggerFactory,
            coroutineContext = Dispatchers.Unconfined,
        )
    }

    @Test
    fun `provideArtist does not retry errored items when backoff not expired`() = runTest {
        // Given: an error state with future tryNextTime (backoff not yet expired)
        val artistId = "artist-123"
        val futureTime = currentTime + 3_600_000 // 1 hour in future
        val errorState = StaticItemFetchState.error(
            itemId = artistId,
            itemType = StaticItemType.Artist,
            errorReason = ErrorReason.Unknown,
            lastAttemptTime = currentTime - 1000,
            tryNextTime = futureTime,
        )

        every { staticsStore.getArtist(artistId) } returns MutableStateFlow(null)
        every { fetchStateStore.get(artistId) } returns MutableStateFlow(errorState)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)

        // When: we request the artist
        val result = staticsProvider.provideArtist(artistId).first()

        // Then: result should be Loading (shows loading while waiting for backoff)
        assertThat(result).isInstanceOf(StaticsItem.Loading::class.java)

        // And: store() should NOT have been called (no retry until backoff expires)
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { staticsSynchronizer.wakeUp() }
    }

    @Test
    fun `provideArtist schedules fetch when error state has past tryNextTime`() = runTest {
        // Given: an error state with tryNextTime in the past (backoff expired)
        val artistId = "artist-123"
        val pastTime = currentTime - 1000 // 1 second ago
        val errorState = StaticItemFetchState.error(
            itemId = artistId,
            itemType = StaticItemType.Artist,
            errorReason = ErrorReason.Unknown,
            lastAttemptTime = currentTime - 60_000,
            tryNextTime = pastTime,
        )

        every { staticsStore.getArtist(artistId) } returns MutableStateFlow(null)
        every { fetchStateStore.get(artistId) } returns MutableStateFlow(errorState)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)

        // When: we request the artist
        val result = staticsProvider.provideArtist(artistId).first()

        // Then: store() SHOULD have been called (new fetch scheduled)
        coVerify(exactly = 1) { fetchStateStore.store(any()) }

        // And: synchronizer SHOULD have been woken up
        verify(exactly = 1) { staticsSynchronizer.wakeUp() }
    }

    @Test
    fun `provideAlbum does not retry errored items when backoff not expired`() = runTest {
        // Given: an error state with future tryNextTime (backoff not yet expired)
        val albumId = "album-456"
        val futureTime = currentTime + 3_600_000 // 1 hour in future
        val errorState = StaticItemFetchState.error(
            itemId = albumId,
            itemType = StaticItemType.Album,
            errorReason = ErrorReason.Network,
            lastAttemptTime = currentTime - 1000,
            tryNextTime = futureTime,
        )

        every { staticsStore.getAlbum(albumId) } returns MutableStateFlow(null)
        every { fetchStateStore.get(albumId) } returns MutableStateFlow(errorState)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)

        // When: we request the album
        val result = staticsProvider.provideAlbum(albumId).first()

        // Then: result should be Loading (shows loading while waiting for backoff)
        assertThat(result).isInstanceOf(StaticsItem.Loading::class.java)

        // And: store() should NOT have been called (no retry until backoff expires)
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { staticsSynchronizer.wakeUp() }
    }

    @Test
    fun `provideTrack does not retry errored items when backoff not expired`() = runTest {
        // Given: an error state with future tryNextTime (backoff not yet expired)
        val trackId = "track-789"
        val futureTime = currentTime + 3_600_000 // 1 hour in future
        val errorState = StaticItemFetchState.error(
            itemId = trackId,
            itemType = StaticItemType.Track,
            errorReason = ErrorReason.NotFound,
            lastAttemptTime = currentTime - 1000,
            tryNextTime = futureTime,
        )

        every { staticsStore.getTrack(trackId) } returns MutableStateFlow(null)
        every { fetchStateStore.get(trackId) } returns MutableStateFlow(errorState)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)

        // When: we request the track
        val result = staticsProvider.provideTrack(trackId).first()

        // Then: result should be Loading (shows loading while waiting for backoff)
        assertThat(result).isInstanceOf(StaticsItem.Loading::class.java)

        // And: store() should NOT have been called (no retry until backoff expires)
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { staticsSynchronizer.wakeUp() }
    }

    @Test
    fun `provideDiscography returns Loaded when skeleton has album IDs`() = runTest {
        // Given: skeleton has album IDs for an artist
        val artistId = "artist-discog-123"
        val albumIds = listOf("album-1", "album-2", "album-3")

        every { skeletonStore.observeAlbumIdsForArtist(artistId) } returns MutableStateFlow(albumIds)

        // When: we request the discography
        val result = staticsProvider.provideDiscography(artistId).first()

        // Then: result should be Loaded with the skeleton album IDs
        assertThat(result).isInstanceOf(StaticsItem.Loaded::class.java)
        val loaded = result as StaticsItem.Loaded
        assertThat(loaded.data.albumsIds).isEqualTo(albumIds)
    }

    @Test
    fun `provideDiscography returns Loaded with empty list when skeleton has no album IDs`() = runTest {
        // Given: skeleton has no data for this artist
        val artistId = "artist-discog-456"

        every { skeletonStore.observeAlbumIdsForArtist(artistId) } returns MutableStateFlow(emptyList())

        // When: we request the discography
        val result = staticsProvider.provideDiscography(artistId).first()

        // Then: result should be Loaded with empty albums list
        assertThat(result).isInstanceOf(StaticsItem.Loaded::class.java)
        val loaded = result as StaticsItem.Loaded
        assertThat(loaded.data.albumsIds).isEmpty()
    }
}
