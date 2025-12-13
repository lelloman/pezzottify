package com.lelloman.pezzottify.android.domain.listening

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ListeningTrackerTest {

    private lateinit var player: PezzottifyPlayer
    private lateinit var listeningEventStore: ListeningEventStore
    private lateinit var listeningEventSynchronizer: ListeningEventSynchronizer
    private lateinit var timeProvider: TimeProvider
    private lateinit var loggerFactory: LoggerFactory

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var tracker: ListeningTracker

    // Player flow controls
    private lateinit var currentTrackIndexFlow: MutableStateFlow<Int?>
    private lateinit var isPlayingFlow: MutableStateFlow<Boolean>
    private lateinit var playbackPlaylistFlow: MutableStateFlow<PlaybackPlaylist?>
    private lateinit var currentTrackDurationSecondsFlow: MutableStateFlow<Int?>
    private lateinit var seekEventsFlow: MutableSharedFlow<ControlsAndStatePlayer.SeekEvent>

    private var currentTime = 1_000_000L

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)

        currentTrackIndexFlow = MutableStateFlow(null)
        isPlayingFlow = MutableStateFlow(false)
        playbackPlaylistFlow = MutableStateFlow(null)
        currentTrackDurationSecondsFlow = MutableStateFlow(null)
        seekEventsFlow = MutableSharedFlow()

        player = mockk(relaxed = true) {
            every { currentTrackIndex } returns currentTrackIndexFlow
            every { isPlaying } returns isPlayingFlow
            every { playbackPlaylist } returns playbackPlaylistFlow
            every { currentTrackDurationSeconds } returns currentTrackDurationSecondsFlow
            every { seekEvents } returns seekEventsFlow
        }

        listeningEventStore = mockk(relaxed = true)
        listeningEventSynchronizer = mockk(relaxed = true)
        timeProvider = TimeProvider { currentTime }

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk {
            every { getLogger(any<String>()) } returns mockLogger
            every { getLogger(any<kotlin.reflect.KClass<*>>()) } returns mockLogger
            every { getValue(any(), any()) } returns mockLogger
        }

        coEvery { listeningEventStore.deleteSyncedEvents() } returns 0
        coEvery { listeningEventStore.saveEvent(any()) } returns 1L
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun setUpPlaylist(trackIds: List<String> = listOf("track-1")) {
        playbackPlaylistFlow.value = PlaybackPlaylist(
            context = PlaybackPlaylistContext.Album("album-1"),
            tracksIds = trackIds,
        )
    }

    // ========== Session Lifecycle Tests ==========

    @Test
    fun `starting playback creates new session`() = runTest {
        // Use backgroundScope for the tracker so it doesn't block test completion
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Simulate time passing while playing
        currentTime += 6_000

        // Wait for periodic save (10 seconds)
        advanceTimeBy(11_000)

        coVerify {
            listeningEventStore.saveEvent(
                withArg { event ->
                    assertThat(event.trackId).isEqualTo("track-1")
                    assertThat(event.trackDurationSeconds).isEqualTo(300)
                }
            )
        }
    }

    @Test
    fun `pausing playback stops duration accumulation`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Advance time while playing
        currentTime += 5_000 // 5 seconds of playback

        // Pause
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        // Wait for periodic save after pause
        advanceTimeBy(11_000)

        coVerify {
            listeningEventStore.saveEvent(
                withArg { event ->
                    assertThat(event.durationSeconds).isEqualTo(5)
                    assertThat(event.pauseCount).isEqualTo(1)
                }
            )
        }
    }

    @Test
    fun `resuming playback continues session`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        currentTime += 5_000 // 5 seconds of playback

        // Pause
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        currentTime += 2_000 // Pause for 2 seconds

        // Resume
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        currentTime += 3_000 // 3 more seconds of playback

        // Wait for periodic save
        advanceTimeBy(11_000)

        coVerify {
            listeningEventStore.saveEvent(
                withArg { event ->
                    // Should have 5 + 3 = 8 seconds of play time, not pause time
                    assertThat(event.durationSeconds).isEqualTo(8)
                    assertThat(event.pauseCount).isEqualTo(1)
                }
            )
        }
    }

    // ========== Periodic Save Tests ==========

    @Test
    fun `periodic save triggers every 10 seconds while playing`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Simulate time passing
        currentTime += 6_000

        // First periodic save at 10 seconds
        advanceTimeBy(11_000)

        coVerify(atLeast = 1) { listeningEventStore.saveEvent(any()) }
        coVerify(atLeast = 1) { listeningEventSynchronizer.wakeUp() }
    }

    @Test
    fun `no periodic save when duration below threshold`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Only 3 seconds elapsed (below 5 second threshold)
        currentTime += 3_000

        // Wait for periodic save interval
        advanceTimeBy(11_000)

        coVerify(exactly = 0) { listeningEventStore.saveEvent(any()) }
    }

    @Test
    fun `no periodic save when paused and already saved`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Play for 10 seconds
        currentTime += 10_000

        // First periodic save
        advanceTimeBy(11_000)

        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }
        coVerify(exactly = 1) { listeningEventSynchronizer.wakeUp() }

        // Pause
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        // Wait for more periodic save intervals while paused
        advanceTimeBy(30_000) // 3 more intervals

        // Should NOT have additional saves or wakeUps since paused and already saved
        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }
        coVerify(exactly = 1) { listeningEventSynchronizer.wakeUp() }
    }

    @Test
    fun `subsequent saves update existing event`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // First save
        currentTime += 10_000
        advanceTimeBy(11_000)

        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }

        // Second save - advance more time while still playing
        currentTime += 10_000
        advanceTimeBy(11_000)

        // Should update existing event instead of saving new one
        coVerify(exactly = 1) { listeningEventStore.updateEvent(any()) }
        coVerify(exactly = 1) { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
    }

    // ========== Track Change Tests ==========

    @Test
    fun `changing track finalizes previous session`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist(listOf("track-1", "track-2"))
        currentTrackDurationSecondsFlow.value = 300

        // Start playing track 1
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        currentTime += 10_000
        advanceTimeBy(11_000) // First save

        // Change to track 2
        currentTrackIndexFlow.value = 1
        testScheduler.runCurrent()

        // First session should be finalized with endedAt set
        coVerify { listeningEventStore.updateEvent(withArg { event ->
            assertThat(event.trackId).isEqualTo("track-1")
            assertThat(event.endedAt).isNotNull()
        }) }
    }

    @Test
    fun `changing track starts new session`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist(listOf("track-1", "track-2"))
        currentTrackDurationSecondsFlow.value = 300

        // Start playing track 1
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        currentTime += 10_000
        advanceTimeBy(11_000) // First save

        // Change to track 2
        currentTrackIndexFlow.value = 1
        testScheduler.runCurrent()

        // Play track 2 for a while
        currentTime += 10_000
        advanceTimeBy(11_000)

        // Should have saved event for track 2
        coVerify { listeningEventStore.saveEvent(withArg { event ->
            assertThat(event.trackId).isEqualTo("track-2")
        }) }
    }

    // ========== Seek Event Tests ==========

    @Test
    fun `seek events increment seek count`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Simulate seek events
        seekEventsFlow.emit(ControlsAndStatePlayer.SeekEvent(currentTime))
        seekEventsFlow.emit(ControlsAndStatePlayer.SeekEvent(currentTime + 1000))
        testScheduler.runCurrent()

        currentTime += 10_000
        advanceTimeBy(11_000)

        coVerify {
            listeningEventStore.saveEvent(
                withArg { event ->
                    assertThat(event.seekCount).isEqualTo(2)
                }
            )
        }
    }

    // ========== Inactivity Timeout Tests ==========

    @Test
    fun `resuming after inactivity timeout starts new session`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        currentTime += 10_000 // 10 seconds of playback
        advanceTimeBy(11_000) // First save

        // Pause
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        // Simulate 5+ minutes of inactivity
        currentTime += 310_000 // 5 minutes + 10 seconds

        // Resume
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Should have finalized old session with endedAt
        coVerify { listeningEventStore.updateEvent(withArg { event ->
            assertThat(event.endedAt).isNotNull()
        }) }
    }

    // ========== Track Duration Update Tests ==========

    @Test
    fun `track duration updates when initially unknown`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        // Duration not available initially
        currentTrackDurationSecondsFlow.value = null

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Duration becomes available
        currentTrackDurationSecondsFlow.value = 300
        testScheduler.runCurrent()

        currentTime += 10_000
        advanceTimeBy(11_000)

        coVerify {
            listeningEventStore.saveEvent(
                withArg { event ->
                    assertThat(event.trackDurationSeconds).isEqualTo(300)
                }
            )
        }
    }

    // ========== Session Cleanup Tests ==========

    @Test
    fun `cleans up synced events when starting new session`() = runTest {
        coEvery { listeningEventStore.deleteSyncedEvents() } returns 5

        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        coVerify { listeningEventStore.deleteSyncedEvents() }
    }

    // ========== Session Discard Tests ==========

    @Test
    fun `session below threshold is discarded on track change`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist(listOf("track-1", "track-2"))
        currentTrackDurationSecondsFlow.value = 300

        // Start playing track 1
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Only 3 seconds (below 5 second threshold)
        currentTime += 3_000

        // Change to track 2 before periodic save
        currentTrackIndexFlow.value = 1
        testScheduler.runCurrent()

        // Track 1 session should not be saved (below threshold)
        coVerify(exactly = 0) { listeningEventStore.saveEvent(any()) }
        coVerify(exactly = 0) { listeningEventStore.updateEvent(any()) }
    }

    // ========== Bug Fix Verification Tests ==========

    @Test
    fun `paused session does not trigger repeated syncs`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Play for 10 seconds
        currentTime += 10_000
        advanceTimeBy(11_000)

        // Verify first save happened
        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }
        coVerify(exactly = 1) { listeningEventSynchronizer.wakeUp() }

        // Pause playback
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        // Wait for multiple periodic save intervals while paused
        // This simulates the bug where updates were happening every 10 seconds even when paused
        advanceTimeBy(50_000) // 5 more intervals

        // Should NOT have additional saves or wakeUp calls since nothing changed
        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }
        coVerify(exactly = 1) { listeningEventSynchronizer.wakeUp() }
        coVerify(exactly = 0) { listeningEventStore.updateEvent(any()) }
    }

    @Test
    fun `resuming after pause triggers new sync`() = runTest {
        tracker = ListeningTracker(
            player = player,
            listeningEventStore = listeningEventStore,
            listeningEventSynchronizer = listeningEventSynchronizer,
            timeProvider = timeProvider,
            scope = backgroundScope,
            loggerFactory = loggerFactory,
        )
        tracker.initialize()

        setUpPlaylist()
        currentTrackDurationSecondsFlow.value = 300

        // Start playing
        currentTrackIndexFlow.value = 0
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Play for 10 seconds
        currentTime += 10_000
        advanceTimeBy(11_000)

        coVerify(exactly = 1) { listeningEventStore.saveEvent(any()) }

        // Pause
        isPlayingFlow.value = false
        testScheduler.runCurrent()

        // Wait while paused - no additional syncs should happen
        advanceTimeBy(20_000)
        coVerify(exactly = 0) { listeningEventStore.updateEvent(any()) }

        // Resume
        isPlayingFlow.value = true
        testScheduler.runCurrent()

        // Play more
        currentTime += 10_000
        advanceTimeBy(11_000)

        // Now an update should have happened since we resumed and played more
        coVerify(atLeast = 1) { listeningEventStore.updateEvent(any()) }
        coVerify(atLeast = 1) { listeningEventStore.updateSyncStatus(1L, SyncStatus.PendingSync) }
    }
}
