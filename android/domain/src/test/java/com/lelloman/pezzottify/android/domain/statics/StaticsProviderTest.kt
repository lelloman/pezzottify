package com.lelloman.pezzottify.android.domain.statics

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.Synchronizer
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
    private lateinit var synchronizer: Synchronizer
    private lateinit var timeProvider: TimeProvider
    private lateinit var loggerFactory: LoggerFactory

    private var currentTime = 1_000_000L

    private lateinit var staticsProvider: StaticsProvider

    @Before
    fun setUp() {
        staticsStore = mockk()
        fetchStateStore = mockk()
        synchronizer = mockk(relaxed = true)
        timeProvider = TimeProvider { currentTime }

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        staticsProvider = StaticsProvider(
            staticsStore = staticsStore,
            staticItemFetchStateStore = fetchStateStore,
            synchronizer = synchronizer,
            timeProvider = timeProvider,
            loggerFactory = loggerFactory,
            coroutineContext = Dispatchers.Unconfined,
        )
    }

    @Test
    fun `provideArtist does NOT schedule fetch when error state has future tryNextTime`() = runTest {
        // Given: an error state with tryNextTime in the future
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

        // Then: result should be Error
        assertThat(result).isInstanceOf(StaticsItem.Error::class.java)

        // And: store() should NOT have been called (no new fetch scheduled)
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { synchronizer.wakeUp() }
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
        verify(exactly = 1) { synchronizer.wakeUp() }
    }

    @Test
    fun `provideAlbum does NOT schedule fetch when error state has future tryNextTime`() = runTest {
        // Given: an error state with tryNextTime in the future
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

        // Then: result should be Error
        assertThat(result).isInstanceOf(StaticsItem.Error::class.java)

        // And: store() should NOT have been called
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { synchronizer.wakeUp() }
    }

    @Test
    fun `provideTrack does NOT schedule fetch when error state has future tryNextTime`() = runTest {
        // Given: an error state with tryNextTime in the future
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

        // Then: result should be Error
        assertThat(result).isInstanceOf(StaticsItem.Error::class.java)

        // And: store() should NOT have been called
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { synchronizer.wakeUp() }
    }

    @Test
    fun `provideDiscography does NOT schedule fetch when error state has future tryNextTime`() = runTest {
        // Given: an error state with tryNextTime in the future
        val artistId = "artist-discog-123"
        val futureTime = currentTime + 3_600_000 // 1 hour in future
        val errorState = StaticItemFetchState.error(
            itemId = artistId,
            itemType = StaticItemType.Discography,
            errorReason = ErrorReason.Client,
            lastAttemptTime = currentTime - 1000,
            tryNextTime = futureTime,
        )

        every { staticsStore.getDiscography(artistId) } returns MutableStateFlow(null)
        every { fetchStateStore.get(artistId) } returns MutableStateFlow(errorState)
        coEvery { fetchStateStore.store(any()) } returns Result.success(Unit)

        // When: we request the discography
        val result = staticsProvider.provideDiscography(artistId).first()

        // Then: result should be Error
        assertThat(result).isInstanceOf(StaticsItem.Error::class.java)

        // And: store() should NOT have been called
        coVerify(exactly = 0) { fetchStateStore.store(any()) }

        // And: synchronizer should NOT have been woken up
        verify(exactly = 0) { synchronizer.wakeUp() }
    }
}
