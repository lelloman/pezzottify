# Listening Stats Plan - Review Document

This document contains all issues, questions, and improvements identified after reviewing the implementation plan against the actual codebase.

---

## Critical Issues

### ISSUE-01: AppInitializer signature mismatch

**Status:** [x] RESOLVED

**Problem:**
The plan showed `initialize(scope: CoroutineScope)` but actual interface is `initialize()`.

**Decision:** Keep current pattern - inject `CoroutineScope` via constructor.

**Rationale:**
- No benefit to changing the interface
- Already testable (pass test scope in constructor)
- Changing would require modifying all existing initializers
- Current injected scope is `GlobalScope` via `AppModule`

---

### ISSUE-02: Should extend BaseSynchronizer instead of custom sync loop

**Status:** [x] RESOLVED

**Decision:** Use `BaseSynchronizer`. The plan pre-dated `BaseSynchronizer`.

**Updated implementation:**
```kotlin
@Singleton
class ListeningEventSynchronizer @Inject internal constructor(
    private val listeningEventStore: ListeningEventStore,
    private val remoteApiClient: RemoteApiClient,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<ListeningEvent>(
    logger = loggerFactory.getLogger(ListeningEventSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {
    override suspend fun getItemsToProcess(): List<ListeningEvent> =
        listeningEventStore.getPendingSyncEvents()

    override suspend fun processItem(item: ListeningEvent) {
        syncEvent(item)
    }

    override suspend fun onBeforeMainLoop() {
        // Cleanup old non-synced events on app startup
        val cutoff = timeProvider.nowUtcMs() - TimeUnit.DAYS.toMillis(CLEANUP_AGE_DAYS)
        listeningEventStore.deleteOldNonSyncedEvents(cutoff)
    }

    private suspend fun syncEvent(event: ListeningEvent) {
        listeningEventStore.updateSyncStatus(event.id, SyncStatus.Syncing)

        val result = remoteApiClient.recordListeningEvent(event.toSyncData())

        when (result) {
            is RemoteApiResponse.Success -> {
                // Delete immediately after successful sync
                listeningEventStore.deleteEvent(event.id)
            }
            is RemoteApiResponse.Error.Network,
            is RemoteApiResponse.Error.Unauthorized -> {
                // Retry later
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
            }
            else -> {
                // Keep retrying (conform to existing pattern)
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.e("Failed to sync listening event: $result")
            }
        }
    }

    companion object {
        private const val MIN_SLEEP_DURATION = 1_000L
        private const val MAX_SLEEP_DURATION = 30_000L
        private const val CLEANUP_AGE_DAYS = 7L
    }
}
```

---

### ISSUE-03: PlaybackContext enum doesn't match actual PlaybackPlaylistContext

**Status:** [x] RESOLVED

**Problem:**
The plan defined a custom `PlaybackContext` enum, but the player already uses `PlaybackPlaylistContext` sealed interface.

**Decision:** Use existing `PlaybackPlaylistContext` with mapping to API strings.

**Purpose:** `playback_context` is used for statistics only (never been used on server yet).

**Mapping:**
```kotlin
fun PlaybackPlaylistContext.toApiString(): String = when (this) {
    is PlaybackPlaylistContext.Album -> "album"  // Album context only exists when unmodified
    is PlaybackPlaylistContext.UserPlaylist -> if (isEdited) "queue" else "playlist"
    is PlaybackPlaylistContext.UserMix -> "queue"
}
```

**Note:** When an Album queue is modified, it becomes `UserMix`. Only `UserPlaylist` has `isEdited` flag.

**Future:** May add "radio" for system-generated playlists.

---

## Architectural Issues

### ISSUE-04: Request model in domain layer violates clean architecture

**Status:** [x] RESOLVED

**Decision:** Option B - Domain DTO

**Implementation:**
```kotlin
// In domain layer: domain/listening/ListeningEventSyncData.kt
data class ListeningEventSyncData(
    val trackId: String,
    val sessionId: String,
    val startedAt: Long,
    val endedAt: Long?,
    val durationSeconds: Int,
    val trackDurationSeconds: Int,
    val seekCount: Int,
    val pauseCount: Int,
    val playbackContext: String,
)

// In domain's RemoteApiClient interface
suspend fun recordListeningEvent(data: ListeningEventSyncData): RemoteApiResponse<ListeningEventResponse>

// In remoteapi's RemoteApiClientImpl - maps DTO to request
override suspend fun recordListeningEvent(
    data: ListeningEventSyncData
): RemoteApiResponse<ListeningEventResponse> = safeApiCall {
    retrofitApiClient.recordListeningEvent(
        ListeningEventRequest(
            trackId = data.trackId,
            sessionId = data.sessionId,
            startedAt = data.startedAt,
            endedAt = data.endedAt,
            durationSeconds = data.durationSeconds,
            trackDurationSeconds = data.trackDurationSeconds,
            seekCount = data.seekCount,
            pauseCount = data.pauseCount,
            playbackContext = data.playbackContext,
            clientType = "android",
        )
    )
}

// Extension in ListeningEvent
fun ListeningEvent.toSyncData() = ListeningEventSyncData(
    trackId = trackId,
    sessionId = sessionId,
    startedAt = startedAt / 1000,  // Convert ms to Unix seconds
    endedAt = endedAt?.let { it / 1000 },
    durationSeconds = durationSeconds,
    trackDurationSeconds = trackDurationSeconds,
    seekCount = seekCount,
    pauseCount = pauseCount,
    playbackContext = playbackContext.toApiString(),
)
```

---

### ISSUE-05: Separate database vs. adding to existing database

**Status:** [x] RESOLVED (partially)

**Decision:** Add to `UserContentDb` (listening events sync to server, similar to liked content).

**Implementation:**
```kotlin
@Database(
    entities = [LikedContentEntity::class, ListeningEventEntity::class],
    version = 2,  // Bump version, add migration
)
internal abstract class UserContentDb : RoomDatabase() {
    abstract fun likedContentDao(): LikedContentDao
    abstract fun listeningEventDao(): ListeningEventDao
}
```

**Open question:** Should we rename `UserDataDb`? Current naming is confusing:
- `UserDataDb` - viewed content, search history (local-only user activity)
- `UserContentDb` - liked content, listening events (synced user content)

Possible rename: `UserDataDb` → `UserActivityDb` or `LocalUserDataDb`?

---

### ISSUE-06: No seek event tracking in player interface

**Status:** [x] RESOLVED

**Decision:** Option A - Add seek events flow to player.

**Implementation:**
```kotlin
// In ControlsAndStatePlayer interface
interface ControlsAndStatePlayer {
    // ... existing members ...

    val seekEvents: SharedFlow<SeekEvent>

    data class SeekEvent(
        val timestamp: Long,
    )
}

// In PlayerImpl - emit when seek happens
private val _seekEvents = MutableSharedFlow<SeekEvent>()
override val seekEvents: SharedFlow<SeekEvent> = _seekEvents

override fun seekToPercentage(percentage: Float) {
    // ... existing implementation ...
    scope.launch {
        _seekEvents.emit(SeekEvent(timestamp = System.currentTimeMillis()))
    }
}

override fun forward10Sec() {
    // ... existing implementation ...
    scope.launch {
        _seekEvents.emit(SeekEvent(timestamp = System.currentTimeMillis()))
    }
}

override fun rewind10Sec() {
    // ... existing implementation ...
    scope.launch {
        _seekEvents.emit(SeekEvent(timestamp = System.currentTimeMillis()))
    }
}
```

**Note:** `SharedFlow` is used instead of `StateFlow` because seeks are events, not state.

---

## Missing Implementations

### ISSUE-07: startNewSession() not implemented

**Status:** [x] RESOLVED

**Problem:**
Track duration was needed but source was unclear.

**Decision:** Add `currentTrackDurationSeconds: StateFlow<Int?>` to the player interface.

**Rationale:**
- Only need current track's duration, not all tracks
- Player already knows which track is playing
- When track is fetched/loaded, duration becomes available
- Simpler than modifying `PlaybackPlaylist` structure

**Implementation:**
```kotlin
// In ControlsAndStatePlayer interface
val currentTrackDurationSeconds: StateFlow<Int?>

// In PlayerImpl - populate when track loads
// (track info comes from StaticsProvider when preparing playback)
```

**Usage in ListeningTracker:**
```kotlin
private suspend fun startNewSession(trackIndex: Int, isPlaying: Boolean) {
    val trackDurationSeconds = player.currentTrackDurationSeconds.value ?: 0
    // ...
}
```

---

### ISSUE-08: calculateFinalDuration() not implemented

**Status:** [x] RESOLVED

**Decision:** Implementation is OK, will be thoroughly unit tested.

**Implementation:**
```kotlin
private fun calculateFinalDuration(session: ActiveSession): Int {
    var totalMs = session.accumulatedDurationMs

    // Add current playing segment if still playing
    session.lastResumeTime?.let { resumeTime ->
        totalMs += timeProvider.nowUtcMs() - resumeTime
    }

    return (totalMs / 1000).toInt()
}
```

---

### ISSUE-09: saveCurrentSessionProgress() not implemented

**Status:** [x] RESOLVED

**Decision:** Keep session concept with periodic saves every 10 seconds.

**Session lifecycle:**
1. **Start:** When a new track begins playing
2. **Continue:** While track plays (accumulating duration, counting pauses/seeks)
3. **Periodic save:** Every 10 seconds, update DB record with current progress
4. **End:** When track changes, playback stops, or after 5 minutes of inactivity

**Inactivity handling:**
- If paused for < 5 minutes: resume continues same session
- If paused for >= 5 minutes: finalize current session on resume, start new one

**Implementation:**
```kotlin
data class ActiveSession(
    // ... existing fields
    var savedEventId: Long? = null,  // DB record ID for updates
)

private suspend fun saveCurrentSessionProgress() {
    val session = currentSession ?: return
    val duration = calculateFinalDuration(session)
    if (duration < MIN_DURATION_THRESHOLD_SEC) return

    val event = createEventFromSession(session, endedAt = null)
    if (session.savedEventId == null) {
        // First save - insert new record
        session.savedEventId = listeningEventStore.saveEvent(event)
    } else {
        // Update existing record
        listeningEventStore.updateEvent(event.copy(id = session.savedEventId!!))
    }
}

private fun shouldStartNewSessionOnResume(): Boolean {
    val session = currentSession ?: return true
    val lastPauseTime = session.lastPauseTime ?: return false
    val pauseDuration = timeProvider.nowUtcMs() - lastPauseTime
    return pauseDuration > INACTIVITY_TIMEOUT_MS  // 5 minutes
}
```

**Constants:**
- `PERIODIC_SAVE_INTERVAL_MS = 10_000` (10 seconds)
- `INACTIVITY_TIMEOUT_MS = 300_000` (5 minutes)

---

### ISSUE-10: Cleanup trigger not implemented

**Status:** [x] RESOLVED

**Decision:**
- Delete synced events immediately after successful sync (not in cleanup)
- Delete non-synced events older than X days in `onBeforeMainLoop()`

**Implementation:** See ISSUE-02 for updated `ListeningEventSynchronizer`.

**Store interface update:**
```kotlin
interface ListeningEventStore {
    // ... existing methods ...

    suspend fun deleteEvent(id: Long)
    suspend fun deleteOldNonSyncedEvents(olderThanMs: Long): Int
    suspend fun deleteAll()  // For logout
}
```

**DAO:**
```kotlin
@Query("DELETE FROM listening_event WHERE id = :id")
suspend fun delete(id: Long)

@Query("DELETE FROM listening_event WHERE sync_status != 'Synced' AND created_at < :olderThanMs")
suspend fun deleteOldNonSynced(olderThanMs: Long): Int

@Query("DELETE FROM listening_event")
suspend fun deleteAll()
```

---

## Inconsistencies

### ISSUE-11: DAO query includes SyncError but sync logic says don't retry

**Status:** [x] RESOLVED

**Decision:** Retry infinitely (conform to existing pattern in codebase).

**Implementation:** All errors result in `PendingSync` status, will be retried. No `SyncError` status used.

```kotlin
when (result) {
    is RemoteApiResponse.Success -> {
        listeningEventStore.deleteEvent(event.id)
    }
    else -> {
        // Always retry
        listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
        if (result is RemoteApiResponse.Error.Unknown) {
            logger.e("Failed to sync listening event: $result")
        }
    }
}
```

**DAO update:**
```kotlin
@Query("SELECT * FROM listening_event WHERE sync_status = 'PendingSync' ORDER BY created_at ASC")
suspend fun getPendingSync(): List<ListeningEventEntity>
```

---

### ISSUE-12: Store method returns Flow but BaseSynchronizer expects suspend function

**Status:** [x] RESOLVED

**Decision:** Change to suspend function as expected by `BaseSynchronizer`.

**Implementation:**
```kotlin
// Store interface
interface ListeningEventStore {
    suspend fun getPendingSyncEvents(): List<ListeningEvent>
    // ...
}

// DAO
@Query("SELECT * FROM listening_event WHERE sync_status = 'PendingSync' ORDER BY created_at ASC")
suspend fun getPendingSync(): List<ListeningEventEntity>
```

---

## Minor Issues

### ISSUE-13: Track duration source unclear

**Status:** [x] RESOLVED

**Decision:** Track duration should come from playback context, not `StaticsStore`.

**Note:** This is linked to ISSUE-07. The implementation depends on how we enhance `PlaybackPlaylist`.

---

### ISSUE-14: UUID import missing

**Status:** [x] RESOLVED

**Decision:** Add the import.

```kotlin
import java.util.UUID
```

---

### ISSUE-15: TypeConverters not needed

**Status:** [x] RESOLVED

**Decision:** Remove `@TypeConverters` annotation - not needed since all fields are primitives/String.

---

### ISSUE-16: Logger injection pattern doesn't match codebase

**Status:** [x] RESOLVED

**Decision:** Use `LoggerFactory` pattern.

```kotlin
class ListeningTracker @Inject constructor(
    // ...
    loggerFactory: LoggerFactory,
) : AppInitializer {
    private val logger = loggerFactory.getLogger(ListeningTracker::class)
}
```

---

### ISSUE-17: CoroutineScope injection for ListeningTracker

**Status:** [x] RESOLVED

**Decision:** Inject scope via constructor.

```kotlin
@Singleton
class ListeningTracker @Inject constructor(
    private val player: PezzottifyPlayer,
    private val listeningEventStore: ListeningEventStore,
    private val timeProvider: TimeProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : AppInitializer {
    private val logger = loggerFactory.getLogger(ListeningTracker::class)

    override fun initialize() {
        // Use injected scope
        scope.launch { /* ... */ }
    }
}
```

---

### ISSUE-18: ListeningTracker needs StaticsStore dependency

**Status:** [x] RESOLVED

**Decision:** No - solve via playback context. Track duration should come from `PlaybackPlaylist`, not `StaticsStore`.

**Note:** Linked to ISSUE-07 and ISSUE-13.

---

### ISSUE-19: Potential race condition in track change handling

**Status:** [x] RESOLVED

**Decision:** Combine flows.

**Implementation:**
```kotlin
data class PlaybackState(
    val trackIndex: Int?,
    val isPlaying: Boolean,
)

override fun initialize() {
    scope.launch {
        combine(
            player.currentTrackIndex,
            player.isPlaying,
        ) { trackIndex, isPlaying ->
            PlaybackState(trackIndex, isPlaying)
        }.collect { state ->
            handlePlaybackStateChange(state)
        }
    }

    // Separate collector for seek events
    scope.launch {
        player.seekEvents.collect {
            onSeekEvent()
        }
    }
}

private var lastTrackIndex: Int? = null

private suspend fun handlePlaybackStateChange(state: PlaybackState) {
    if (state.trackIndex != lastTrackIndex) {
        // Track changed
        currentSession?.let { finalizeSession(it) }
        if (state.trackIndex != null) {
            startNewSession(state.trackIndex, state.isPlaying)
        }
        lastTrackIndex = state.trackIndex
    } else {
        // Same track, play state changed
        onPlayStateChanged(state.isPlaying)
    }
}
```

---

### ISSUE-20: What happens when user logs out?

**Status:** [x] RESOLVED

**Decision:** Delete everything on logout.

**Implementation:**
Add to logout flow (in `PerformLogout` use case or similar):
```kotlin
listeningEventStore.deleteAll()
```

---

## Summary Checklist

| ID | Issue | Severity | Status |
|----|-------|----------|--------|
| ISSUE-01 | AppInitializer signature | Critical | [x] RESOLVED - Keep constructor injection |
| ISSUE-02 | Use BaseSynchronizer | Critical | [x] RESOLVED |
| ISSUE-03 | PlaybackContext mismatch | Critical | [x] RESOLVED - Map to album/playlist/queue |
| ISSUE-04 | Request model location | High | [x] RESOLVED - Domain DTO |
| ISSUE-05 | Database location | High | [x] RESOLVED - UserContentDb, rename UserDataDb |
| ISSUE-06 | Seek tracking | Medium | [x] RESOLVED - Add to player |
| ISSUE-07 | startNewSession() impl | Medium | [x] RESOLVED - currentTrackDurationSeconds |
| ISSUE-08 | calculateFinalDuration() impl | Medium | [x] RESOLVED |
| ISSUE-09 | saveCurrentSessionProgress() impl | Medium | [x] RESOLVED - Periodic saves every 10s |
| ISSUE-10 | Cleanup trigger | Medium | [x] RESOLVED - Delete on sync + cleanup old |
| ISSUE-11 | SyncError retry policy | Medium | [x] RESOLVED - Retry infinitely |
| ISSUE-12 | Flow vs suspend return | Medium | [x] RESOLVED - Suspend function |
| ISSUE-13 | Track duration source | Low | [x] RESOLVED - currentTrackDurationSeconds |
| ISSUE-14 | UUID import | Low | [x] RESOLVED |
| ISSUE-15 | TypeConverters not needed | Low | [x] RESOLVED |
| ISSUE-16 | Logger pattern | Low | [x] RESOLVED - LoggerFactory |
| ISSUE-17 | CoroutineScope injection | Low | [x] RESOLVED - Constructor injection |
| ISSUE-18 | StaticsStore dependency | Low | [x] RESOLVED - Not needed |
| ISSUE-19 | Race condition | Low | [x] RESOLVED - Combine flows |
| ISSUE-20 | Logout handling | Low | [x] RESOLVED - Delete all |

---

## All Questions Resolved

### Q1: AppInitializer scope parameter (ISSUE-01)
**Decision:** Keep current pattern - inject scope via constructor. No benefit to changing the interface, and it's already testable.

### Q2: playback_context purpose (ISSUE-03)
**Decision:** Used for statistics only. Mapping:
- Clean album playback → "album"
- Clean user playlist → "playlist"
- UserMix or modified queue → "queue"
- Future: may add "radio" for system-generated playlists

### Q3: Rename UserDataDb (ISSUE-05)
**Decision:** Rename to `UserLocalDataDb` to clarify it contains local-only data (viewed content, search history) vs `UserContentDb` which syncs to server.

### Q4: Track duration source (ISSUE-07, ISSUE-13, ISSUE-18)
**Decision:** Add `currentTrackDurationSeconds: StateFlow<Int?>` to player. When track is fetched/loaded, populate this field. Only track current track's duration, not all tracks in playlist.

### Q5: Session lifecycle and periodic saves (ISSUE-09)
**Decision:** Keep session concept with periodic saves:
- **Session start:** When track starts playing
- **Session end:** When track changes, playback stops, or after N minutes of inactivity
- **Periodic saves:** Every 10 seconds, update existing DB record with current progress
- **Inactivity timeout:** After 5 minutes of pause, finalize current session; resume creates new session
- **Data structure:** `ActiveSession` gets a `savedEventId: Long?` to track the DB record ID for updates
