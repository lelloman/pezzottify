package com.lelloman.pezzottify.android.domain.usercontent

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.coVerifyOrder
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.flowOf
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
class UserContentSynchronizerTest {

    private lateinit var userContentStore: UserContentStore
    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var loggerFactory: LoggerFactory

    private val testDispatcher = UnconfinedTestDispatcher()
    private lateinit var testScope: CoroutineScope

    private lateinit var synchronizer: UserContentSynchronizer

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        testScope = CoroutineScope(testDispatcher)

        userContentStore = mockk(relaxed = true)
        remoteApiClient = mockk(relaxed = true)

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getLogger(any<kotlin.reflect.KClass<*>>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        // Default behavior: no pending items
        coEvery { userContentStore.getPendingSyncItems() } returns flowOf(emptyList())
    }

    @After
    fun tearDown() {
        testScope.cancel()
        Dispatchers.resetMain()
    }

    private fun createSynchronizer(): UserContentSynchronizer {
        return UserContentSynchronizer(
            userContentStore = userContentStore,
            remoteApiClient = remoteApiClient,
            loggerFactory = loggerFactory,
            dispatcher = testDispatcher,
            scope = testScope,
        )
    }

    // ========== Main Loop Sleep/Wake Tests ==========

    @Test
    fun `main loop goes to sleep when no pending items`() = runTest {
        var getPendingCallCount = 0
        coEvery { userContentStore.getPendingSyncItems() } answers {
            getPendingCallCount++
            flowOf(emptyList())
        }

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // Should have called getPendingSyncItems once and then gone to sleep
        assertThat(getPendingCallCount).isEqualTo(1)
    }

    @Test
    fun `main loop wakes up when wakeUp is called`() = runTest {
        var callCount = 0
        coEvery { userContentStore.getPendingSyncItems() } answers {
            callCount++
            flowOf(emptyList())
        }

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        assertThat(callCount).isEqualTo(1)

        // Wake up the synchronizer
        synchronizer.wakeUp()
        advanceUntilIdle()

        // Should have called getPendingSyncItems again after waking up
        assertThat(callCount).isEqualTo(2)
    }

    @Test
    fun `main loop continues iterating when there are pending items`() = runTest {
        var iterationCount = 0
        val pendingItem = createLikedContent("item-1", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } answers {
            iterationCount++
            if (iterationCount <= 2) {
                flowOf(listOf(pendingItem))
            } else {
                flowOf(emptyList())
            }
        }
        coEvery { remoteApiClient.likeContent(any()) } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceTimeBy(35_000) // Allow time for multiple iterations with backoff
        advanceUntilIdle()

        // Should have iterated multiple times due to pending items
        assertThat(iterationCount).isAtLeast(2)
    }

    // ========== Sync Success Tests ==========

    @Test
    fun `sync success for like marks item as Synced`() = runTest {
        val item = createLikedContent("content-123", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-123") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.likeContent("content-123") }
        coVerify { userContentStore.updateSyncStatus("content-123", SyncStatus.Synced) }
    }

    @Test
    fun `sync success for unlike marks item as Synced`() = runTest {
        val item = createLikedContent("content-456", isLiked = false)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.unlikeContent("content-456") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.unlikeContent("content-456") }
        coVerify { userContentStore.updateSyncStatus("content-456", SyncStatus.Synced) }
    }

    @Test
    fun `sync sets Syncing status before making API call`() = runTest {
        val item = createLikedContent("content-789", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-789") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerifyOrder {
            userContentStore.updateSyncStatus("content-789", SyncStatus.Syncing)
            remoteApiClient.likeContent("content-789")
            userContentStore.updateSyncStatus("content-789", SyncStatus.Synced)
        }
    }

    // ========== Error Handling Tests ==========

    @Test
    fun `network error keeps item as PendingSync for retry`() = runTest {
        val item = createLikedContent("content-network", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-network") } returns RemoteApiResponse.Error.Network

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { userContentStore.updateSyncStatus("content-network", SyncStatus.Syncing) }
        coVerify { userContentStore.updateSyncStatus("content-network", SyncStatus.PendingSync) }
    }

    @Test
    fun `unauthorized error marks item as SyncError`() = runTest {
        val item = createLikedContent("content-unauth", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-unauth") } returns RemoteApiResponse.Error.Unauthorized

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { userContentStore.updateSyncStatus("content-unauth", SyncStatus.Syncing) }
        coVerify { userContentStore.updateSyncStatus("content-unauth", SyncStatus.SyncError) }
    }

    @Test
    fun `not found error marks item as SyncError`() = runTest {
        val item = createLikedContent("content-notfound", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-notfound") } returns RemoteApiResponse.Error.NotFound

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { userContentStore.updateSyncStatus("content-notfound", SyncStatus.Syncing) }
        coVerify { userContentStore.updateSyncStatus("content-notfound", SyncStatus.SyncError) }
    }

    @Test
    fun `unknown error marks item as SyncError`() = runTest {
        val item = createLikedContent("content-unknown", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("content-unknown") } returns RemoteApiResponse.Error.Unknown("Something went wrong")

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { userContentStore.updateSyncStatus("content-unknown", SyncStatus.Syncing) }
        coVerify { userContentStore.updateSyncStatus("content-unknown", SyncStatus.SyncError) }
    }

    // ========== Multiple Items Tests ==========

    @Test
    fun `processes multiple pending items in single iteration`() = runTest {
        val items = listOf(
            createLikedContent("item-1", isLiked = true),
            createLikedContent("item-2", isLiked = false),
            createLikedContent("item-3", isLiked = true),
        )

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(items),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent(any()) } returns RemoteApiResponse.Success(Unit)
        coEvery { remoteApiClient.unlikeContent(any()) } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { remoteApiClient.likeContent("item-1") }
        coVerify { remoteApiClient.unlikeContent("item-2") }
        coVerify { remoteApiClient.likeContent("item-3") }

        coVerify { userContentStore.updateSyncStatus("item-1", SyncStatus.Synced) }
        coVerify { userContentStore.updateSyncStatus("item-2", SyncStatus.Synced) }
        coVerify { userContentStore.updateSyncStatus("item-3", SyncStatus.Synced) }
    }

    @Test
    fun `error in one item does not prevent processing of other items`() = runTest {
        val items = listOf(
            createLikedContent("item-fail", isLiked = true),
            createLikedContent("item-success", isLiked = true),
        )

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(items),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("item-fail") } returns RemoteApiResponse.Error.NotFound
        coEvery { remoteApiClient.likeContent("item-success") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        // Both should be processed
        coVerify { remoteApiClient.likeContent("item-fail") }
        coVerify { remoteApiClient.likeContent("item-success") }

        // Failure should be marked as error, success should be synced
        coVerify { userContentStore.updateSyncStatus("item-fail", SyncStatus.SyncError) }
        coVerify { userContentStore.updateSyncStatus("item-success", SyncStatus.Synced) }
    }

    // ========== Like vs Unlike API Call Tests ==========

    @Test
    fun `isLiked true calls likeContent API`() = runTest {
        val item = createLikedContent("like-item", isLiked = true)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.likeContent("like-item") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 1) { remoteApiClient.likeContent("like-item") }
        coVerify(exactly = 0) { remoteApiClient.unlikeContent(any()) }
    }

    @Test
    fun `isLiked false calls unlikeContent API`() = runTest {
        val item = createLikedContent("unlike-item", isLiked = false)

        coEvery { userContentStore.getPendingSyncItems() } returnsMany listOf(
            flowOf(listOf(item)),
            flowOf(emptyList())
        )
        coEvery { remoteApiClient.unlikeContent("unlike-item") } returns RemoteApiResponse.Success(Unit)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 0) { remoteApiClient.likeContent(any()) }
        coVerify(exactly = 1) { remoteApiClient.unlikeContent("unlike-item") }
    }

    // ========== fetchRemoteLikedContent Tests ==========

    @Test
    fun `fetchRemoteLikedContent adds remote items not in local DB`() = runTest {
        // Remote has item-1, item-2
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Success(listOf("item-1", "item-2"))
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())

        // Local has no items
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Album)) } returns flowOf(emptyList())
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()
        synchronizer.fetchRemoteLikedContent()

        // Both remote items should be added to local
        coVerify {
            userContentStore.setLiked(
                contentId = "item-1",
                type = LikedContent.ContentType.Album,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced
            )
        }
        coVerify {
            userContentStore.setLiked(
                contentId = "item-2",
                type = LikedContent.ContentType.Album,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced
            )
        }
    }

    @Test
    fun `fetchRemoteLikedContent keeps existing local items`() = runTest {
        // Remote has item-1
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Success(listOf("item-1"))
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())

        // Local already has item-1
        val localItem = createLikedContent("item-1", isLiked = true, type = LikedContent.ContentType.Album)
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Album)) } returns flowOf(listOf(localItem))
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()
        synchronizer.fetchRemoteLikedContent()

        // Should NOT call setLiked for item-1 since it already exists locally
        coVerify(exactly = 0) {
            userContentStore.setLiked(
                contentId = "item-1",
                type = LikedContent.ContentType.Album,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced
            )
        }
    }

    @Test
    fun `fetchRemoteLikedContent removes unliked items when synced but not on server`() = runTest {
        // Remote has no items
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Success(emptyList())
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())

        // Local has item-1 marked as liked and synced (meaning it was synced but removed on another device)
        val localItem = createLikedContent(
            "item-1",
            isLiked = true,
            type = LikedContent.ContentType.Album,
            syncStatus = SyncStatus.Synced
        )
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Album)) } returns flowOf(listOf(localItem))
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()
        synchronizer.fetchRemoteLikedContent()

        // Should mark local item as unliked since it's not on server
        coVerify {
            userContentStore.setLiked(
                contentId = "item-1",
                type = LikedContent.ContentType.Album,
                liked = false,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced
            )
        }
    }

    @Test
    fun `fetchRemoteLikedContent does not remove pending sync items not on server`() = runTest {
        // Remote has no items
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Success(emptyList())
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())

        // Local has item-1 marked as liked but pending sync (local change not yet synced)
        val localItem = createLikedContent(
            "item-1",
            isLiked = true,
            type = LikedContent.ContentType.Album,
            syncStatus = SyncStatus.PendingSync
        )
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Album)) } returns flowOf(listOf(localItem))
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()
        synchronizer.fetchRemoteLikedContent()

        // Should NOT mark local pending item as unliked
        coVerify(exactly = 0) {
            userContentStore.setLiked(
                contentId = "item-1",
                type = any(),
                liked = false,
                modifiedAt = any(),
                syncStatus = any()
            )
        }
    }

    @Test
    fun `fetchRemoteLikedContent handles network error gracefully`() = runTest {
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Error.Network
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()

        // Should not throw exception
        synchronizer.fetchRemoteLikedContent()

        // Artist fetch should still proceed
        coVerify { remoteApiClient.getLikedContent("artist") }
    }

    @Test
    fun `fetchRemoteLikedContent handles unauthorized error gracefully`() = runTest {
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Error.Unauthorized
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())
        coEvery { userContentStore.getLikedContent(listOf(LikedContent.ContentType.Artist)) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()

        // Should not throw exception
        synchronizer.fetchRemoteLikedContent()

        // Artist fetch should still proceed
        coVerify { remoteApiClient.getLikedContent("artist") }
    }

    @Test
    fun `fetchRemoteLikedContent fetches both album and artist types`() = runTest {
        coEvery { remoteApiClient.getLikedContent("album") } returns RemoteApiResponse.Success(emptyList())
        coEvery { remoteApiClient.getLikedContent("artist") } returns RemoteApiResponse.Success(emptyList())
        coEvery { userContentStore.getLikedContent(any()) } returns flowOf(emptyList())

        synchronizer = createSynchronizer()
        synchronizer.fetchRemoteLikedContent()

        coVerify { remoteApiClient.getLikedContent("album") }
        coVerify { remoteApiClient.getLikedContent("artist") }
    }

    // ========== Helper Functions ==========

    private fun createLikedContent(
        contentId: String,
        isLiked: Boolean,
        type: LikedContent.ContentType = LikedContent.ContentType.Album,
        syncStatus: SyncStatus = SyncStatus.PendingSync,
    ): LikedContent {
        return object : LikedContent {
            override val contentId: String = contentId
            override val contentType: LikedContent.ContentType = type
            override val isLiked: Boolean = isLiked
            override val modifiedAt: Long = System.currentTimeMillis()
            override val syncStatus: SyncStatus = syncStatus
        }
    }
}
