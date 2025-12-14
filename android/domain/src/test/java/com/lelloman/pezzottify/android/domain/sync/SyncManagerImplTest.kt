package com.lelloman.pezzottify.android.domain.sync

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.LikesState
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.logger.Logger
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.time.Duration

@OptIn(ExperimentalCoroutinesApi::class)
class SyncManagerImplTest {

    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var syncStateStore: SyncStateStore
    private lateinit var userContentStore: UserContentStore
    private lateinit var userPlaylistStore: UserPlaylistStore
    private lateinit var permissionsStore: PermissionsStore
    private lateinit var userSettingsStore: UserSettingsStore
    private lateinit var downloadStatusRepository: DownloadStatusRepository
    private lateinit var notificationRepository: NotificationRepository
    private lateinit var logger: Logger

    private val testDispatcher = StandardTestDispatcher()
    private val testScope = TestScope(testDispatcher)

    private lateinit var syncManager: SyncManagerImpl

    @Before
    fun setUp() {
        remoteApiClient = mockk(relaxed = true)
        syncStateStore = mockk(relaxed = true)
        userContentStore = mockk(relaxed = true)
        userPlaylistStore = mockk(relaxed = true)
        permissionsStore = mockk(relaxed = true)
        userSettingsStore = mockk(relaxed = true)
        downloadStatusRepository = mockk(relaxed = true)
        notificationRepository = mockk(relaxed = true)
        logger = mockk(relaxed = true)

        // Default: no full sync needed (tests can override)
        every { syncStateStore.needsFullSync() } returns false

        syncManager = SyncManagerImpl(
            remoteApiClient = remoteApiClient,
            syncStateStore = syncStateStore,
            userContentStore = userContentStore,
            userPlaylistStore = userPlaylistStore,
            permissionsStore = permissionsStore,
            userSettingsStore = userSettingsStore,
            downloadStatusRepository = downloadStatusRepository,
            notificationRepository = notificationRepository,
            logger = logger,
            dispatcher = testDispatcher,
            scope = testScope,
            // Use infinite retry delays to disable retries in tests
            minRetryDelay = Duration.INFINITE,
            maxRetryDelay = Duration.INFINITE,
        )
    }

    @After
    fun tearDown() {
        // Advance any pending coroutines (retries are disabled so this is safe)
        testScope.testScheduler.advanceUntilIdle()
    }

    // region initialize

    @Test
    fun `initialize performs fullSync when cursor is 0`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 0L
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L)
        )

        val result = syncManager.initialize()

        assertThat(result).isTrue()
        coVerify { remoteApiClient.getSyncState() }
        coVerify { syncStateStore.saveCursor(10L) }
    }

    @Test
    fun `initialize performs catchUp when cursor is greater than 0`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = emptyList(), currentSeq = 5L)
        )

        val result = syncManager.initialize()

        assertThat(result).isTrue()
        coVerify { remoteApiClient.getSyncEvents(5L) }
    }

    // endregion

    // region fullSync

    @Test
    fun `fullSync success updates state to Synced and saves cursor`() = runTest(testDispatcher) {
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 42L)
        )

        val result = syncManager.fullSync()

        assertThat(result).isTrue()
        assertThat(syncManager.state.value).isEqualTo(SyncState.Synced(42L))
        coVerify { syncStateStore.saveCursor(42L) }
    }

    @Test
    fun `fullSync applies likes state to user content store`() = runTest(testDispatcher) {
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(
                seq = 10L,
                likesAlbums = listOf("album1", "album2"),
                likesArtists = listOf("artist1"),
                likesTracks = listOf("track1", "track2", "track3"),
            )
        )

        syncManager.fullSync()

        coVerify(exactly = 2) {
            userContentStore.setLiked(
                contentId = match { it in listOf("album1", "album2") },
                type = LikedContent.ContentType.Album,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
        coVerify(exactly = 1) {
            userContentStore.setLiked(
                contentId = "artist1",
                type = LikedContent.ContentType.Artist,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
        coVerify(exactly = 3) {
            userContentStore.setLiked(
                contentId = match { it in listOf("track1", "track2", "track3") },
                type = LikedContent.ContentType.Track,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
    }

    @Test
    fun `fullSync error updates state to Error`() = runTest(testDispatcher) {
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Error.Network

        val result = syncManager.fullSync()

        assertThat(result).isFalse()
        assertThat(syncManager.state.value).isInstanceOf(SyncState.Error::class.java)
        assertThat((syncManager.state.value as SyncState.Error).message).isEqualTo("Network error")
    }

    @Test
    fun `fullSync sets state to Syncing during operation`() = runTest(testDispatcher) {
        var statesDuringSync = mutableListOf<SyncState>()
        coEvery { remoteApiClient.getSyncState() } answers {
            statesDuringSync.add(syncManager.state.value)
            RemoteApiResponse.Success(createSyncStateResponse(seq = 10L))
        }

        syncManager.fullSync()

        assertThat(statesDuringSync).contains(SyncState.Syncing)
    }

    // endregion

    // region catchUp

    @Test
    fun `catchUp success applies events and updates cursor`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createContentLikedEvent(seq = 6L, contentId = "track1"),
            createContentLikedEvent(seq = 7L, contentId = "track2"),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 7L)
        )

        val result = syncManager.catchUp()

        assertThat(result).isTrue()
        assertThat(syncManager.state.value).isEqualTo(SyncState.Synced(7L))
        coVerify { syncStateStore.saveCursor(6L) }
        coVerify { syncStateStore.saveCursor(7L) }
    }

    @Test
    fun `catchUp applies content liked events to user content store`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createContentLikedEvent(seq = 6L, contentId = "track1", contentType = LikedContentType.Track),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 6L)
        )

        syncManager.catchUp()

        coVerify {
            userContentStore.setLiked(
                contentId = "track1",
                type = LikedContent.ContentType.Track,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
    }

    @Test
    fun `catchUp applies content unliked events to user content store`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createContentUnlikedEvent(seq = 6L, contentId = "album1", contentType = LikedContentType.Album),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 6L)
        )

        syncManager.catchUp()

        coVerify {
            userContentStore.setLiked(
                contentId = "album1",
                type = LikedContent.ContentType.Album,
                liked = false,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
    }

    @Test
    fun `catchUp triggers fullSync when events are pruned`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Error.EventsPruned
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 100L)
        )

        val result = syncManager.catchUp()

        assertThat(result).isTrue()
        coVerify { remoteApiClient.getSyncState() }
        coVerify { syncStateStore.saveCursor(100L) }
    }

    @Test
    fun `catchUp triggers fullSync when sequence gap detected`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        // Gap: cursor is 5, but first event is 10 (should be 6)
        val events = listOf(
            createContentLikedEvent(seq = 10L, contentId = "track1"),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 10L)
        )
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L)
        )

        val result = syncManager.catchUp()

        assertThat(result).isTrue()
        coVerify { remoteApiClient.getSyncState() }
    }

    @Test
    fun `catchUp error updates state to Error`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Error.Unauthorized

        val result = syncManager.catchUp()

        assertThat(result).isFalse()
        assertThat(syncManager.state.value).isInstanceOf(SyncState.Error::class.java)
    }

    @Test
    fun `catchUp with no events updates cursor to currentSeq`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = emptyList(), currentSeq = 5L)
        )

        val result = syncManager.catchUp()

        assertThat(result).isTrue()
        assertThat(syncManager.state.value).isEqualTo(SyncState.Synced(5L))
    }

    @Test
    fun `catchUp updates cursor even when currentSeq is higher than last event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = emptyList(), currentSeq = 10L)
        )

        syncManager.catchUp()

        coVerify { syncStateStore.saveCursor(10L) }
    }

    // endregion

    // region handleSyncMessage

    @Test
    fun `handleSyncMessage applies event and updates cursor`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val event = createContentLikedEvent(seq = 6L, contentId = "track1")

        syncManager.handleSyncMessage(event)

        coVerify { syncStateStore.saveCursor(6L) }
        assertThat(syncManager.state.value).isEqualTo(SyncState.Synced(6L))
    }

    @Test
    fun `handleSyncMessage triggers catchUp when sequence gap detected`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val event = createContentLikedEvent(seq = 10L, contentId = "track1")
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = emptyList(), currentSeq = 10L)
        )

        syncManager.handleSyncMessage(event)

        coVerify { remoteApiClient.getSyncEvents(5L) }
    }

    @Test
    fun `handleSyncMessage applies content liked event to user content store`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val event = createContentLikedEvent(
            seq = 6L,
            contentId = "artist1",
            contentType = LikedContentType.Artist,
        )

        syncManager.handleSyncMessage(event)

        coVerify {
            userContentStore.setLiked(
                contentId = "artist1",
                type = LikedContent.ContentType.Artist,
                liked = true,
                modifiedAt = any(),
                syncStatus = SyncStatus.Synced,
            )
        }
    }

    // endregion

    // region cleanup

    @Test
    fun `cleanup clears cursor and resets state to Idle`() = runTest(testDispatcher) {
        // First sync to get a non-Idle state
        every { syncStateStore.getCurrentCursor() } returns 0L
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L)
        )
        syncManager.initialize()
        assertThat(syncManager.state.value).isInstanceOf(SyncState.Synced::class.java)

        syncManager.cleanup()

        coVerify { syncStateStore.clearCursor() }
        coVerify { downloadStatusRepository.clear() }
        assertThat(syncManager.state.value).isEqualTo(SyncState.Idle)
    }

    // endregion

    // region settings

    @Test
    fun `fullSync applies settings from response`() = runTest(testDispatcher) {
        val settings = listOf(
            UserSetting.ExternalSearchEnabled(true)
        )
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L, settings = settings)
        )

        syncManager.fullSync()

        coVerify { userSettingsStore.setExternalSearchEnabled(true) }
    }

    @Test
    fun `handleSyncMessage applies setting changed event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val event = createSettingChangedEvent(
            seq = 6L,
            setting = UserSetting.ExternalSearchEnabled(true),
        )

        syncManager.handleSyncMessage(event)

        coVerify { userSettingsStore.setExternalSearchEnabled(true) }
        coVerify { syncStateStore.saveCursor(6L) }
    }

    @Test
    fun `catchUp applies setting changed events`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createSettingChangedEvent(seq = 6L, setting = UserSetting.ExternalSearchEnabled(false)),
            createSettingChangedEvent(seq = 7L, setting = UserSetting.ExternalSearchEnabled(true)),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 7L)
        )

        syncManager.catchUp()

        coVerify { userSettingsStore.setExternalSearchEnabled(false) }
        coVerify { userSettingsStore.setExternalSearchEnabled(true) }
    }

    // endregion

    // region helper functions

    private fun createSyncStateResponse(
        seq: Long = 0L,
        likesAlbums: List<String> = emptyList(),
        likesArtists: List<String> = emptyList(),
        likesTracks: List<String> = emptyList(),
        settings: List<UserSetting> = emptyList(),
    ): SyncStateResponse {
        return SyncStateResponse(
            seq = seq,
            likes = LikesState(
                albums = likesAlbums,
                artists = likesArtists,
                tracks = likesTracks,
            ),
            settings = settings,
            playlists = emptyList(),
            permissions = emptyList(),
        )
    }

    private fun createContentLikedEvent(
        seq: Long,
        contentId: String,
        contentType: LikedContentType = LikedContentType.Track,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "content_liked",
            payload = SyncEventPayload(
                contentType = contentType,
                contentId = contentId,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    private fun createContentUnlikedEvent(
        seq: Long,
        contentId: String,
        contentType: LikedContentType = LikedContentType.Track,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "content_unliked",
            payload = SyncEventPayload(
                contentType = contentType,
                contentId = contentId,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    private fun createSettingChangedEvent(
        seq: Long,
        setting: UserSetting,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "setting_changed",
            payload = SyncEventPayload(
                setting = setting,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    // endregion
}
