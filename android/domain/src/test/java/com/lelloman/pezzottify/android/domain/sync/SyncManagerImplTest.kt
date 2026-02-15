package com.lelloman.pezzottify.android.domain.sync

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.notifications.DownloadCompletedData
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.notifications.NotificationType
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import io.mockk.verify
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.LikesState
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.PlaylistState
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylist
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
import kotlinx.coroutines.test.advanceTimeBy
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
    private lateinit var systemNotificationHelper: SystemNotificationHelper
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
        systemNotificationHelper = mockk(relaxed = true)
        logger = mockk(relaxed = true)

        // Default: no full sync needed (tests can override)
        every { syncStateStore.needsFullSync() } returns false

        // Default: no pending playlists (tests can override)
        every { userPlaylistStore.getPendingSyncPlaylists() } returns kotlinx.coroutines.flow.flowOf(emptyList())

        syncManager = SyncManagerImpl(
            remoteApiClient = remoteApiClient,
            syncStateStore = syncStateStore,
            userContentStore = userContentStore,
            userPlaylistStore = userPlaylistStore,
            permissionsStore = permissionsStore,
            userSettingsStore = userSettingsStore,
            downloadStatusRepository = downloadStatusRepository,
            notificationRepository = notificationRepository,
            systemNotificationHelper = systemNotificationHelper,
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

    // region fullSync playlists

    @Test
    fun `fullSync replaces all playlists with server state`() = runTest(testDispatcher) {
        val serverPlaylists = listOf(
            PlaylistState(id = "pl-1", name = "Playlist One", tracks = listOf("t1", "t2")),
            PlaylistState(id = "pl-2", name = "Playlist Two", tracks = listOf("t3")),
        )
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L, playlists = serverPlaylists)
        )

        syncManager.fullSync()

        val playlistsSlot = slot<List<UserPlaylist>>()
        coVerify { userPlaylistStore.replaceAllPlaylists(capture(playlistsSlot)) }
        val replaced = playlistsSlot.captured
        assertThat(replaced).hasSize(2)
        assertThat(replaced[0].id).isEqualTo("pl-1")
        assertThat(replaced[0].name).isEqualTo("Playlist One")
        assertThat(replaced[0].trackIds).containsExactly("t1", "t2").inOrder()
        assertThat(replaced[0].syncStatus).isEqualTo(PlaylistSyncStatus.Synced)
        assertThat(replaced[1].id).isEqualTo("pl-2")
    }

    @Test
    fun `fullSync should preserve locally pending playlists during replacement`() = runTest(testDispatcher) {
        // Local playlist has been modified (PendingUpdate) with locally-added tracks
        val localPlaylist = object : UserPlaylist {
            override val id = "pl-1"
            override val name = "My Playlist"
            override val trackIds = listOf("t1", "t2", "t3-local")
            override val syncStatus = PlaylistSyncStatus.PendingUpdate
        }
        coEvery { userPlaylistStore.getPendingSyncPlaylists() } returns kotlinx.coroutines.flow.flowOf(
            listOf(localPlaylist)
        )

        // Server has the same playlist but without the locally-added track
        val serverPlaylists = listOf(
            PlaylistState(id = "pl-1", name = "My Playlist", tracks = listOf("t1", "t2")),
        )
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L, playlists = serverPlaylists)
        )

        syncManager.fullSync()

        // The locally-modified playlist's tracks should be preserved
        val playlistsSlot = slot<List<UserPlaylist>>()
        coVerify { userPlaylistStore.replaceAllPlaylists(capture(playlistsSlot)) }
        val replaced = playlistsSlot.captured
        val playlist = replaced.find { it.id == "pl-1" }!!
        assertThat(playlist.trackIds).contains("t3-local")
        assertThat(playlist.syncStatus).isEqualTo(PlaylistSyncStatus.PendingUpdate)
    }

    @Test
    fun `fullSync should preserve PendingCreate playlists that are not on server`() = runTest(testDispatcher) {
        // Local playlist was just created, server doesn't know about it yet
        val localPlaylist = object : UserPlaylist {
            override val id = "local-pl-new"
            override val name = "Brand New Playlist"
            override val trackIds = listOf("t1")
            override val syncStatus = PlaylistSyncStatus.PendingCreate
        }
        coEvery { userPlaylistStore.getPendingSyncPlaylists() } returns kotlinx.coroutines.flow.flowOf(
            listOf(localPlaylist)
        )

        // Server has no playlists
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L, playlists = emptyList())
        )

        syncManager.fullSync()

        // The PendingCreate playlist should survive the replaceAll
        val playlistsSlot = slot<List<UserPlaylist>>()
        coVerify { userPlaylistStore.replaceAllPlaylists(capture(playlistsSlot)) }
        val replaced = playlistsSlot.captured
        val localResult = replaced.find { it.id == "local-pl-new" }
        assertThat(localResult).isNotNull()
        assertThat(localResult!!.name).isEqualTo("Brand New Playlist")
        assertThat(localResult.trackIds).containsExactly("t1")
        assertThat(localResult.syncStatus).isEqualTo(PlaylistSyncStatus.PendingCreate)
    }

    // endregion

    // region catchUp playlists

    @Test
    fun `catchUp applies playlist_created event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createPlaylistCreatedEvent(seq = 6L, playlistId = "pl-new", name = "New Playlist"),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 6L)
        )

        syncManager.catchUp()

        coVerify {
            userPlaylistStore.createOrUpdatePlaylist(
                id = "pl-new",
                name = "New Playlist",
                trackIds = emptyList(),
            )
        }
    }

    @Test
    fun `catchUp applies playlist_deleted event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createPlaylistDeletedEvent(seq = 6L, playlistId = "pl-gone"),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 6L)
        )

        syncManager.catchUp()

        coVerify { userPlaylistStore.deletePlaylist("pl-gone") }
    }

    @Test
    fun `catchUp applies playlist_tracks_updated event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createPlaylistTracksUpdatedEvent(
                seq = 6L,
                playlistId = "pl-1",
                trackIds = listOf("t1", "t2", "t3"),
            ),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 6L)
        )

        syncManager.catchUp()

        coVerify {
            userPlaylistStore.updatePlaylistTracks(
                playlistId = "pl-1",
                trackIds = listOf("t1", "t2", "t3"),
                fromServer = true,
            )
        }
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
            UserSetting.NotifyWhatsNew(true)
        )
        coEvery { remoteApiClient.getSyncState() } returns RemoteApiResponse.Success(
            createSyncStateResponse(seq = 10L, settings = settings)
        )

        syncManager.fullSync()

        coVerify { userSettingsStore.setNotifyWhatsNewEnabled(true) }
    }

    @Test
    fun `handleSyncMessage applies setting changed event`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val event = createSettingChangedEvent(
            seq = 6L,
            setting = UserSetting.NotifyWhatsNew(true),
        )

        syncManager.handleSyncMessage(event)

        coVerify { userSettingsStore.setNotifyWhatsNewEnabled(true) }
        coVerify { syncStateStore.saveCursor(6L) }
    }

    @Test
    fun `catchUp applies setting changed events`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L
        val events = listOf(
            createSettingChangedEvent(seq = 6L, setting = UserSetting.NotifyWhatsNew(false)),
            createSettingChangedEvent(seq = 7L, setting = UserSetting.NotifyWhatsNew(true)),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 7L)
        )

        syncManager.catchUp()

        coVerify { userSettingsStore.setNotifyWhatsNewEnabled(false) }
        coVerify { userSettingsStore.setNotifyWhatsNewEnabled(true) }
    }

    // endregion

    // region showSystemNotificationsForUnread

    @Test
    fun `showSystemNotificationsForUnread shows grouped notification for unread recent downloads`() {
        val data = DownloadCompletedData(
            albumId = "album-1",
            albumName = "Test Album",
            artistName = "Test Artist",
            imageId = null,
            requestId = "req-1",
        )
        val notification = createDownloadNotification("notif-1", data)

        syncManager.showSystemNotificationsForUnread(listOf(notification))

        val downloadsSlot = slot<List<DownloadCompletedData>>()
        val idsSlot = slot<List<String>>()
        verify { systemNotificationHelper.showDownloadsCompletedNotification(capture(downloadsSlot), capture(idsSlot)) }
        assertThat(downloadsSlot.captured).hasSize(1)
        assertThat(downloadsSlot.captured[0].albumId).isEqualTo("album-1")
        assertThat(downloadsSlot.captured[0].albumName).isEqualTo("Test Album")
        assertThat(idsSlot.captured).containsExactly("notif-1")
    }

    @Test
    fun `showSystemNotificationsForUnread groups multiple downloads into one call`() {
        val data1 = DownloadCompletedData(
            albumId = "album-1", albumName = "Album A", artistName = "Artist A",
            imageId = null, requestId = "req-1",
        )
        val data2 = DownloadCompletedData(
            albumId = "album-2", albumName = "Album B", artistName = "Artist B",
            imageId = null, requestId = "req-2",
        )

        syncManager.showSystemNotificationsForUnread(listOf(
            createDownloadNotification("notif-1", data1),
            createDownloadNotification("notif-2", data2),
        ))

        val downloadsSlot = slot<List<DownloadCompletedData>>()
        val idsSlot = slot<List<String>>()
        verify { systemNotificationHelper.showDownloadsCompletedNotification(capture(downloadsSlot), capture(idsSlot)) }
        assertThat(downloadsSlot.captured).hasSize(2)
        assertThat(idsSlot.captured).containsExactly("notif-1", "notif-2")
    }

    @Test
    fun `showSystemNotificationsForUnread skips read notifications`() {
        val data = DownloadCompletedData(
            albumId = "album-1", albumName = "Test Album", artistName = "Test Artist",
            imageId = null, requestId = "req-1",
        )
        val notification = createDownloadNotification("notif-1", data, readAt = System.currentTimeMillis())

        syncManager.showSystemNotificationsForUnread(listOf(notification))

        verify(exactly = 0) { systemNotificationHelper.showDownloadsCompletedNotification(any(), any()) }
    }

    @Test
    fun `showSystemNotificationsForUnread skips old notifications`() {
        val data = DownloadCompletedData(
            albumId = "album-1", albumName = "Test Album", artistName = "Test Artist",
            imageId = null, requestId = "req-1",
        )
        val twoDaysAgo = System.currentTimeMillis() - 2 * 24 * 60 * 60 * 1000L
        val notification = createDownloadNotification("notif-1", data, createdAt = twoDaysAgo)

        syncManager.showSystemNotificationsForUnread(listOf(notification))

        verify(exactly = 0) { systemNotificationHelper.showDownloadsCompletedNotification(any(), any()) }
    }

    // endregion

    // region download notification debouncing

    @Test
    fun `catchUp debounces download notifications into grouped call`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L

        val data1 = DownloadCompletedData(
            albumId = "album-1", albumName = "Album A", artistName = "Artist A",
            imageId = null, requestId = "req-1",
        )
        val data2 = DownloadCompletedData(
            albumId = "album-2", albumName = "Album B", artistName = "Artist B",
            imageId = null, requestId = "req-2",
        )

        val events = listOf(
            createNotificationCreatedEvent(seq = 6L, data = data1),
            createNotificationCreatedEvent(seq = 7L, data = data2),
        )
        coEvery { remoteApiClient.getSyncEvents(5L) } returns RemoteApiResponse.Success(
            SyncEventsResponse(events = events, currentSeq = 7L)
        )

        syncManager.catchUp()

        // Before debounce fires, no notification shown
        verify(exactly = 0) { systemNotificationHelper.showDownloadsCompletedNotification(any(), any()) }

        // Advance past the debounce window
        testScope.advanceTimeBy(3000)

        val downloadsSlot = slot<List<DownloadCompletedData>>()
        val idsSlot = slot<List<String>>()
        verify { systemNotificationHelper.showDownloadsCompletedNotification(capture(downloadsSlot), capture(idsSlot)) }
        assertThat(downloadsSlot.captured).hasSize(2)
        assertThat(downloadsSlot.captured[0].albumId).isEqualTo("album-1")
        assertThat(downloadsSlot.captured[1].albumId).isEqualTo("album-2")
        assertThat(idsSlot.captured).containsExactly("notif-6", "notif-7")
    }

    @Test
    fun `handleSyncMessage shows single download notification via grouped method`() = runTest(testDispatcher) {
        every { syncStateStore.getCurrentCursor() } returns 5L

        val data = DownloadCompletedData(
            albumId = "album-1", albumName = "Test Album", artistName = "Test Artist",
            imageId = null, requestId = "req-1",
        )
        val event = createNotificationCreatedEvent(seq = 6L, data = data)

        syncManager.handleSyncMessage(event)
        testScope.advanceTimeBy(3000)

        val downloadsSlot = slot<List<DownloadCompletedData>>()
        val idsSlot = slot<List<String>>()
        verify { systemNotificationHelper.showDownloadsCompletedNotification(capture(downloadsSlot), capture(idsSlot)) }
        assertThat(downloadsSlot.captured).hasSize(1)
        assertThat(idsSlot.captured).hasSize(1)
    }

    // endregion

    // region helper functions

    private fun createSyncStateResponse(
        seq: Long = 0L,
        likesAlbums: List<String> = emptyList(),
        likesArtists: List<String> = emptyList(),
        likesTracks: List<String> = emptyList(),
        settings: List<UserSetting> = emptyList(),
        playlists: List<PlaylistState> = emptyList(),
        notifications: List<com.lelloman.pezzottify.android.domain.notifications.Notification> = emptyList(),
    ): SyncStateResponse {
        return SyncStateResponse(
            seq = seq,
            likes = LikesState(
                albums = likesAlbums,
                artists = likesArtists,
                tracks = likesTracks,
            ),
            settings = settings,
            playlists = playlists,
            permissions = emptyList(),
            notifications = notifications,
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

    private fun createPlaylistCreatedEvent(
        seq: Long,
        playlistId: String,
        name: String,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "playlist_created",
            payload = SyncEventPayload(
                playlistId = playlistId,
                name = name,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    private fun createPlaylistDeletedEvent(
        seq: Long,
        playlistId: String,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "playlist_deleted",
            payload = SyncEventPayload(
                playlistId = playlistId,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    private fun createPlaylistTracksUpdatedEvent(
        seq: Long,
        playlistId: String,
        trackIds: List<String>,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        return StoredEvent(
            seq = seq,
            type = "playlist_tracks_updated",
            payload = SyncEventPayload(
                playlistId = playlistId,
                trackIds = trackIds,
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

    private fun createNotificationCreatedEvent(
        seq: Long,
        data: DownloadCompletedData,
        serverTimestamp: Long = System.currentTimeMillis(),
    ): StoredEvent {
        val notification = createDownloadNotification("notif-$seq", data)
        return StoredEvent(
            seq = seq,
            type = "notification_created",
            payload = SyncEventPayload(
                notification = notification,
            ),
            serverTimestamp = serverTimestamp,
        )
    }

    private fun createDownloadNotification(
        id: String,
        data: DownloadCompletedData,
        readAt: Long? = null,
        createdAt: Long = System.currentTimeMillis(),
    ): com.lelloman.pezzottify.android.domain.notifications.Notification {
        return com.lelloman.pezzottify.android.domain.notifications.Notification(
            id = id,
            notificationType = NotificationType.DownloadCompleted,
            title = "${data.albumName} is ready",
            body = "by ${data.artistName}",
            data = kotlinx.serialization.json.Json.encodeToJsonElement(
                DownloadCompletedData.serializer(), data
            ),
            readAt = readAt,
            createdAt = createdAt,
        )
    }

    // endregion
}
