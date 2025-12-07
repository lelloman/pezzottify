package com.lelloman.pezzottify.android.domain.listening

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventRecordedResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalCoroutinesApi::class)
class ListeningEventSynchronizerTest {

    private lateinit var listeningEventStore: ListeningEventStore
    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var timeProvider: TimeProvider
    private lateinit var loggerFactory: LoggerFactory

    private var currentTime = 1_000_000L

    private val testDispatcher = UnconfinedTestDispatcher()
    private lateinit var testScope: CoroutineScope

    private lateinit var synchronizer: ListeningEventSynchronizer

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        testScope = CoroutineScope(testDispatcher)

        listeningEventStore = mockk(relaxed = true)
        remoteApiClient = mockk(relaxed = true)
        timeProvider = TimeProvider { currentTime }

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getLogger(any<kotlin.reflect.KClass<*>>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        // Default behavior: no pending events
        coEvery { listeningEventStore.getPendingSyncEvents() } returns emptyList()
        coEvery { listeningEventStore.deleteOldNonSyncedEvents(any()) } returns 0
    }

    @After
    fun tearDown() {
        testScope.cancel()
        Dispatchers.resetMain()
    }

    private fun createSynchronizer(): ListeningEventSynchronizer {
        return ListeningEventSynchronizer(
            listeningEventStore = listeningEventStore,
            remoteApiClient = remoteApiClient,
            timeProvider = timeProvider,
            loggerFactory = loggerFactory,
            dispatcher = testDispatcher,
            scope = testScope,
        )
    }

    // ========== Initialization Tests ==========

    @Test
    fun `initialize cleans up old non-synced events`() = runTest {
        synchronizer = createSynchronizer()

        synchronizer.initialize()
        advanceUntilIdle()

        val expectedCutoff = currentTime - TimeUnit.DAYS.toMillis(7)
        coVerify { listeningEventStore.deleteOldNonSyncedEvents(expectedCutoff) }
    }

    @Test
    fun `initialize can only be called once`() = runTest {
        synchronizer = createSynchronizer()

        synchronizer.initialize()
        advanceUntilIdle()

        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 1) { listeningEventStore.deleteOldNonSyncedEvents(any()) }
    }

    // ========== Main Loop Sleep/Wake Tests ==========

    @Test
    fun `main loop goes to sleep when no pending events`() = runTest {
        var callCount = 0
        coEvery { listeningEventStore.getPendingSyncEvents() } answers {
            callCount++
            emptyList()
        }

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        assertThat(callCount).isEqualTo(1)
    }

    @Test
    fun `main loop wakes up when wakeUp is called`() = runTest {
        var callCount = 0
        coEvery { listeningEventStore.getPendingSyncEvents() } answers {
            callCount++
            emptyList()
        }

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        assertThat(callCount).isEqualTo(1)

        synchronizer.wakeUp()
        advanceUntilIdle()

        assertThat(callCount).isEqualTo(2)
    }

    // ========== Sync Success Tests ==========

    @Test
    fun `successful sync marks event as synced`() = runTest {
        val event = createTestEvent(id = 1L)
        val response = ListeningEventRecordedResponse(id = 100L, created = true)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            listOf(event),
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Success(response)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.Syncing) }
        coVerify { remoteApiClient.recordListeningEvent(any()) }
        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.Synced) }
    }

    @Test
    fun `sync sends correct data to API`() = runTest {
        val event = createTestEvent(
            id = 1L,
            trackId = "track-123",
            sessionId = "session-456",
            startedAt = 1000000L,
            endedAt = 1000300L,
            durationSeconds = 300,
            trackDurationSeconds = 320,
            seekCount = 5,
            pauseCount = 2,
        )
        val response = ListeningEventRecordedResponse(id = 100L, created = true)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            listOf(event),
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Success(response)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify {
            remoteApiClient.recordListeningEvent(
                withArg { data ->
                    assertThat(data.trackId).isEqualTo("track-123")
                    assertThat(data.sessionId).isEqualTo("session-456")
                    assertThat(data.startedAt).isEqualTo(1000L) // Converted from ms to seconds
                    assertThat(data.endedAt).isEqualTo(1000L)   // Converted from ms to seconds
                    assertThat(data.durationSeconds).isEqualTo(300)
                    assertThat(data.trackDurationSeconds).isEqualTo(320)
                    assertThat(data.seekCount).isEqualTo(5)
                    assertThat(data.pauseCount).isEqualTo(2)
                    assertThat(data.playbackContext).isEqualTo("album")
                }
            )
        }
    }

    // ========== Error Handling Tests ==========

    @Test
    fun `network error reverts to pending sync status`() = runTest {
        val event = createTestEvent(id = 1L)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            listOf(event),
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Error.Network

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.Syncing) }
        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
        coVerify(exactly = 0) { listeningEventStore.deleteEvent(1L) }
    }

    @Test
    fun `unauthorized error reverts to pending sync status`() = runTest {
        val event = createTestEvent(id = 1L)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            listOf(event),
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Error.Unauthorized

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
        coVerify(exactly = 0) { listeningEventStore.deleteEvent(1L) }
    }

    @Test
    fun `unknown error reverts to pending sync status`() = runTest {
        val event = createTestEvent(id = 1L)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            listOf(event),
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Error.Unknown("Server error")

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
        coVerify(exactly = 0) { listeningEventStore.deleteEvent(1L) }
    }

    // ========== Multiple Events Tests ==========

    @Test
    fun `processes multiple pending events`() = runTest {
        val events = listOf(
            createTestEvent(id = 1L, sessionId = "session-1"),
            createTestEvent(id = 2L, sessionId = "session-2"),
            createTestEvent(id = 3L, sessionId = "session-3"),
        )
        val response = ListeningEventRecordedResponse(id = 100L, created = true)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            events,
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(any()) } returns RemoteApiResponse.Success(response)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 3) { remoteApiClient.recordListeningEvent(any()) }
        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.Synced) }
        coVerify { listeningEventStore.updateSyncStatus(2L, SyncStatus.Synced) }
        coVerify { listeningEventStore.updateSyncStatus(3L, SyncStatus.Synced) }
    }

    @Test
    fun `error in one event does not prevent processing other events`() = runTest {
        val events = listOf(
            createTestEvent(id = 1L, sessionId = "session-fail"),
            createTestEvent(id = 2L, sessionId = "session-success"),
        )
        val response = ListeningEventRecordedResponse(id = 100L, created = true)

        coEvery { listeningEventStore.getPendingSyncEvents() } returnsMany listOf(
            events,
            emptyList()
        )
        coEvery { remoteApiClient.recordListeningEvent(match { it.sessionId == "session-fail" }) } returns RemoteApiResponse.Error.Network
        coEvery { remoteApiClient.recordListeningEvent(match { it.sessionId == "session-success" }) } returns RemoteApiResponse.Success(response)

        synchronizer = createSynchronizer()
        synchronizer.initialize()
        advanceUntilIdle()

        coVerify(exactly = 2) { remoteApiClient.recordListeningEvent(any()) }
        coVerify { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
        coVerify { listeningEventStore.updateSyncStatus(2L, SyncStatus.Synced) }
    }

    // ========== Helper Functions ==========

    private fun createTestEvent(
        id: Long = 1L,
        trackId: String = "track-1",
        sessionId: String = "session-1",
        startedAt: Long = 1000000L,
        endedAt: Long? = 1000300L,
        durationSeconds: Int = 300,
        trackDurationSeconds: Int = 320,
        seekCount: Int = 0,
        pauseCount: Int = 0,
        playbackContext: PlaybackPlaylistContext = PlaybackPlaylistContext.Album("album-1"),
        syncStatus: SyncStatus = SyncStatus.PendingSync,
        createdAt: Long = 1000000L,
    ): ListeningEvent {
        return ListeningEvent(
            id = id,
            trackId = trackId,
            sessionId = sessionId,
            startedAt = startedAt,
            endedAt = endedAt,
            durationSeconds = durationSeconds,
            trackDurationSeconds = trackDurationSeconds,
            seekCount = seekCount,
            pauseCount = pauseCount,
            playbackContext = playbackContext,
            syncStatus = syncStatus,
            createdAt = createdAt,
        )
    }
}
