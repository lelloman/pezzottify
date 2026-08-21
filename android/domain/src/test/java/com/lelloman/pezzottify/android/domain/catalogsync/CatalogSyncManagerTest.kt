package com.lelloman.pezzottify.android.domain.catalogsync

import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.coVerifyOrder
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class CatalogSyncManagerTest {

    private val cursor = MutableStateFlow(5L)
    private lateinit var catalogSyncStore: CatalogSyncStore
    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var staticsStore: StaticsStore
    private lateinit var manager: CatalogSyncManager

    @Before
    fun setUp() {
        catalogSyncStore = mockk(relaxed = true)
        remoteApiClient = mockk()
        staticsStore = mockk(relaxed = true)
        every { catalogSyncStore.currentSeq } returns cursor

        val logger = mockk<Logger>(relaxed = true)
        val loggerFactory = mockk<LoggerFactory>()
        every { loggerFactory.getLogger(any<String>()) } returns logger
        every { loggerFactory.getLogger(any<kotlin.reflect.KClass<*>>()) } returns logger
        every { loggerFactory.getValue(any(), any()) } returns logger

        manager = CatalogSyncManager(
            catalogSyncStore = catalogSyncStore,
            staticsStore = staticsStore,
            staticsCache = mockk<StaticsCache>(relaxed = true),
            remoteApiClient = remoteApiClient,
            loggerFactory = loggerFactory,
        )
    }

    @Test
    fun `complete page advances cursor to server sequence`() = runTest {
        coEvery { remoteApiClient.getCatalogSync(5) } returns RemoteApiResponse.Success(
            CatalogSyncResponse(
                events = emptyList(),
                currentSeq = 9,
                hasMore = false,
                nextSince = 9,
            ),
        )

        manager.catchUp()

        coVerify(exactly = 1) { remoteApiClient.getCatalogSync(5) }
        coVerify(exactly = 1) { catalogSyncStore.setCurrentSeq(9) }
    }

    @Test
    fun `network failure leaves cursor unchanged`() = runTest {
        coEvery { remoteApiClient.getCatalogSync(5) } returns RemoteApiResponse.Error.Network

        manager.catchUp()

        coVerify(exactly = 0) { catalogSyncStore.setCurrentSeq(any()) }
    }

    @Test
    fun `pruned events reset cursor`() = runTest {
        coEvery { remoteApiClient.getCatalogSync(5) } returns RemoteApiResponse.Error.EventsPruned

        manager.catchUp()

        coVerify(exactly = 1) { catalogSyncStore.setCurrentSeq(0) }
    }

    @Test
    fun `catch up applies every page and checkpoints each cursor in order`() = runTest {
        val firstEvent = trackEvent(seq = 6, contentId = "track-6")
        val secondEvent = trackEvent(seq = 7, contentId = "track-7")
        coEvery { remoteApiClient.getCatalogSync(any()) } returnsMany listOf(
            RemoteApiResponse.Success(
                CatalogSyncResponse(
                    events = listOf(firstEvent),
                    currentSeq = 9,
                    hasMore = true,
                    nextSince = 6,
                ),
            ),
            RemoteApiResponse.Success(
                CatalogSyncResponse(
                    events = listOf(secondEvent),
                    currentSeq = 9,
                    hasMore = false,
                    nextSince = 9,
                ),
            ),
        )

        manager.catchUp()

        coVerifyOrder {
            remoteApiClient.getCatalogSync(5)
            staticsStore.deleteTrack("track-6")
            catalogSyncStore.setCurrentSeq(6)
            remoteApiClient.getCatalogSync(6)
            staticsStore.deleteTrack("track-7")
            catalogSyncStore.setCurrentSeq(9)
        }
    }

    @Test
    fun `later page failure preserves the last completed page cursor`() = runTest {
        coEvery { remoteApiClient.getCatalogSync(any()) } returnsMany listOf(
            RemoteApiResponse.Success(
                CatalogSyncResponse(
                    events = listOf(trackEvent(seq = 6, contentId = "track-6")),
                    currentSeq = 9,
                    hasMore = true,
                    nextSince = 6,
                ),
            ),
            RemoteApiResponse.Error.Network,
        )

        manager.catchUp()

        coVerifyOrder {
            remoteApiClient.getCatalogSync(5)
            staticsStore.deleteTrack("track-6")
            catalogSyncStore.setCurrentSeq(6)
            remoteApiClient.getCatalogSync(6)
        }
        coVerify(exactly = 0) { catalogSyncStore.setCurrentSeq(9) }
    }

    @Test
    fun `non advancing page stops without changing the cursor`() = runTest {
        coEvery { remoteApiClient.getCatalogSync(5) } returns RemoteApiResponse.Success(
            CatalogSyncResponse(
                events = emptyList(),
                currentSeq = 9,
                hasMore = true,
                nextSince = 5,
            ),
        )

        manager.catchUp()

        coVerify(exactly = 1) { remoteApiClient.getCatalogSync(5) }
        coVerify(exactly = 0) { catalogSyncStore.setCurrentSeq(any()) }
    }

    private fun trackEvent(seq: Long, contentId: String) = CatalogEvent(
        seq = seq,
        eventType = CatalogEventType.TrackUpdated,
        contentType = CatalogContentType.Track,
        contentId = contentId,
        timestamp = 1_700_000_000,
    )
}
