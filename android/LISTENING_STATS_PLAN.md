# Listening Stats Feature - Android Implementation Plan

## Overview

Implement client-side listening stats tracking that integrates with the server API. The client captures playback events from the player, stores them locally for offline support, and syncs to the server via a background synchronizer.

## Server API Reference

### POST `/v1/user/listening`

**Request Body (Minimal):**
```json
{
  "track_id": "tra_xxxxx",
  "duration_seconds": 187,
  "track_duration_seconds": 210
}
```

**Request Body (Full):**
```json
{
  "track_id": "tra_xxxxx",
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "started_at": 1732982400,
  "ended_at": 1732982587,
  "duration_seconds": 187,
  "track_duration_seconds": 210,
  "seek_count": 2,
  "pause_count": 1,
  "playback_context": "album",
  "client_type": "android"
}
```

**Response:**
```json
{ "id": 42, "created": true }
```

**Deduplication:** If `session_id` already exists on server, returns `{ "id": <existing>, "created": false }` (idempotent for offline queue retry).

### GET `/v1/user/listening/summary?start_date=YYYYMMDD&end_date=YYYYMMDD`

Returns user's listening summary for date range.

### GET `/v1/user/listening/history?limit=50`

Returns recently played tracks with play counts.

---

## Architecture

Following existing patterns from `UserContentSynchronizer` and liked content flow:

```
Player Events (PlayerImpl)
    ↓
ListeningTracker (monitors playback state)
    ↓
LogListeningEventUseCase
    ↓
ListeningEventStore.saveEvent(syncStatus=PendingSync)
    ↓ (Room Database)
ListeningEventEntity
    ↓
ListeningEventSynchronizer (background loop)
    ↓
RemoteApiClient.recordListeningEvent()
    ↓
Server API
```

---

## 1. Domain Layer

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEvent.kt`

```kotlin
data class ListeningEvent(
    val id: Long = 0,
    val trackId: String,
    val sessionId: String,
    val startedAt: Long,        // Unix timestamp ms
    val endedAt: Long?,         // Unix timestamp ms, null if still playing
    val durationSeconds: Int,   // Actual listening time (excluding pauses)
    val trackDurationSeconds: Int,
    val seekCount: Int = 0,
    val pauseCount: Int = 0,
    val playbackContext: PlaybackContext,
    val syncStatus: SyncStatus = SyncStatus.PendingSync,
    val createdAt: Long,
)

enum class PlaybackContext {
    Album,
    Playlist,
    Track,
    Search,
    Queue,
    Unknown;

    fun toApiString(): String = name.lowercase()
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventStore.kt`

```kotlin
interface ListeningEventStore {
    suspend fun saveEvent(event: ListeningEvent)

    suspend fun updateEvent(event: ListeningEvent)

    fun getPendingSyncEvents(): Flow<List<ListeningEvent>>

    suspend fun updateSyncStatus(id: Long, status: SyncStatus)

    suspend fun getActiveSession(trackId: String): ListeningEvent?

    suspend fun deleteOldSyncedEvents(olderThanMs: Long)
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt`

```kotlin
/**
 * Tracks playback state and generates listening events.
 *
 * Triggers event recording when:
 * - Track changes (finalize previous track's event)
 * - Playback stops/pauses for extended period
 * - App goes to background
 * - Minimum threshold reached (5 seconds)
 */
@Singleton
class ListeningTracker @Inject constructor(
    private val player: PezzottifyPlayer,
    private val listeningEventStore: ListeningEventStore,
    private val timeProvider: TimeProvider,
    private val logger: Logger,
) : AppInitializer {

    private var currentSession: ActiveSession? = null

    data class ActiveSession(
        val sessionId: String,
        val trackId: String,
        val trackDurationSeconds: Int,
        val startedAt: Long,
        var accumulatedDurationMs: Long = 0,
        var lastResumeTime: Long? = null,
        var seekCount: Int = 0,
        var pauseCount: Int = 0,
        val playbackContext: PlaybackContext,
    )

    override fun initialize(scope: CoroutineScope) {
        // Monitor track changes
        scope.launch {
            player.currentTrackIndex.collect { trackIndex ->
                onTrackChanged(trackIndex)
            }
        }

        // Monitor play/pause state
        scope.launch {
            player.isPlaying.collect { isPlaying ->
                onPlayStateChanged(isPlaying)
            }
        }

        // Periodic save for long sessions (every 30 seconds)
        scope.launch {
            while (true) {
                delay(30_000)
                saveCurrentSessionProgress()
            }
        }
    }

    private suspend fun onTrackChanged(newTrackIndex: Int?) {
        // Finalize previous session
        currentSession?.let { session ->
            finalizeSession(session)
        }

        // Start new session if track is playing
        if (newTrackIndex != null) {
            startNewSession()
        }
    }

    private suspend fun onPlayStateChanged(isPlaying: Boolean) {
        val session = currentSession ?: return

        if (isPlaying) {
            session.lastResumeTime = timeProvider.nowUtcMs()
        } else {
            // Accumulate duration on pause
            session.lastResumeTime?.let { resumeTime ->
                session.accumulatedDurationMs += timeProvider.nowUtcMs() - resumeTime
                session.lastResumeTime = null
                session.pauseCount++
            }
        }
    }

    private suspend fun finalizeSession(session: ActiveSession) {
        // Calculate final duration
        val finalDuration = calculateFinalDuration(session)

        // Only save if minimum threshold met (5 seconds)
        if (finalDuration >= MIN_DURATION_THRESHOLD_SEC) {
            val event = ListeningEvent(
                trackId = session.trackId,
                sessionId = session.sessionId,
                startedAt = session.startedAt,
                endedAt = timeProvider.nowUtcMs(),
                durationSeconds = finalDuration,
                trackDurationSeconds = session.trackDurationSeconds,
                seekCount = session.seekCount,
                pauseCount = session.pauseCount,
                playbackContext = session.playbackContext,
                createdAt = timeProvider.nowUtcMs(),
            )
            listeningEventStore.saveEvent(event)
        }

        currentSession = null
    }

    companion object {
        const val MIN_DURATION_THRESHOLD_SEC = 5
    }
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventSynchronizer.kt`

```kotlin
/**
 * Background synchronizer that sends pending listening events to server.
 *
 * Follows same pattern as UserContentSynchronizer:
 * - Exponential backoff on failures
 * - Session ID deduplication handles retries
 * - Network errors trigger retry, client errors don't
 */
@Singleton
class ListeningEventSynchronizer @Inject constructor(
    private val listeningEventStore: ListeningEventStore,
    private val remoteApiClient: RemoteApiClient,
    private val logger: Logger,
) : AppInitializer {

    private var wakeUpSignal = CompletableDeferred<Unit>()
    private var sleepDuration = MIN_SLEEP_DURATION

    override fun initialize(scope: CoroutineScope) {
        scope.launch(Dispatchers.IO) {
            mainLoop()
        }
    }

    private suspend fun mainLoop() {
        while (true) {
            val pendingEvents = listeningEventStore.getPendingSyncEvents().first()

            if (pendingEvents.isEmpty()) {
                wakeUpSignal = CompletableDeferred()
                wakeUpSignal.await()
                sleepDuration = MIN_SLEEP_DURATION
                continue
            }

            for (event in pendingEvents) {
                syncEvent(event)
            }

            delay(sleepDuration)
            sleepDuration = (sleepDuration * 1.4).toLong()
                .coerceAtMost(MAX_SLEEP_DURATION)
        }
    }

    private suspend fun syncEvent(event: ListeningEvent) {
        listeningEventStore.updateSyncStatus(event.id, SyncStatus.Syncing)

        val result = remoteApiClient.recordListeningEvent(
            ListeningEventRequest(
                trackId = event.trackId,
                sessionId = event.sessionId,
                startedAt = event.startedAt / 1000, // Convert to Unix seconds
                endedAt = event.endedAt?.let { it / 1000 },
                durationSeconds = event.durationSeconds,
                trackDurationSeconds = event.trackDurationSeconds,
                seekCount = event.seekCount,
                pauseCount = event.pauseCount,
                playbackContext = event.playbackContext.toApiString(),
                clientType = "android",
            )
        )

        when (result) {
            is RemoteApiResponse.Success -> {
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.Synced)
            }
            is RemoteApiResponse.Error.Network -> {
                // Retry later
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                // User logged out, keep for later
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
            }
            else -> {
                // Client error, don't retry
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.SyncError)
                logger.e("Failed to sync listening event: $result")
            }
        }
    }

    fun wakeUp() {
        wakeUpSignal.complete(Unit)
    }

    companion object {
        private const val MIN_SLEEP_DURATION = 1_000L
        private const val MAX_SLEEP_DURATION = 30_000L
    }
}
```

---

## 2. Local Data Layer

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/model/ListeningEventEntity.kt`

```kotlin
@Entity(
    tableName = "listening_event",
    indices = [
        Index("sync_status"),
        Index("session_id", unique = true),
    ]
)
data class ListeningEventEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "track_id") val trackId: String,
    @ColumnInfo(name = "session_id") val sessionId: String,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "ended_at") val endedAt: Long?,
    @ColumnInfo(name = "duration_seconds") val durationSeconds: Int,
    @ColumnInfo(name = "track_duration_seconds") val trackDurationSeconds: Int,
    @ColumnInfo(name = "seek_count") val seekCount: Int,
    @ColumnInfo(name = "pause_count") val pauseCount: Int,
    @ColumnInfo(name = "playback_context") val playbackContext: String,
    @ColumnInfo(name = "sync_status") val syncStatus: String,
    @ColumnInfo(name = "created_at") val createdAt: Long,
)
```

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/ListeningEventDao.kt`

```kotlin
@Dao
interface ListeningEventDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(event: ListeningEventEntity): Long

    @Update
    suspend fun update(event: ListeningEventEntity)

    @Query("SELECT * FROM listening_event WHERE sync_status IN ('PendingSync', 'SyncError') ORDER BY created_at ASC")
    fun getPendingSync(): Flow<List<ListeningEventEntity>>

    @Query("UPDATE listening_event SET sync_status = :status WHERE id = :id")
    suspend fun updateSyncStatus(id: Long, status: String)

    @Query("SELECT * FROM listening_event WHERE track_id = :trackId AND ended_at IS NULL LIMIT 1")
    suspend fun getActiveSession(trackId: String): ListeningEventEntity?

    @Query("SELECT * FROM listening_event WHERE session_id = :sessionId LIMIT 1")
    suspend fun getBySessionId(sessionId: String): ListeningEventEntity?

    @Query("DELETE FROM listening_event WHERE sync_status = 'Synced' AND created_at < :olderThanMs")
    suspend fun deleteOldSynced(olderThanMs: Long): Int

    @Query("SELECT COUNT(*) FROM listening_event WHERE sync_status = 'PendingSync'")
    fun getPendingCount(): Flow<Int>
}
```

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/ListeningEventDb.kt`

```kotlin
@Database(
    entities = [ListeningEventEntity::class],
    version = 1,
    exportSchema = true,
)
@TypeConverters(ListeningEventTypeConverters::class)
abstract class ListeningEventDb : RoomDatabase() {
    abstract fun listeningEventDao(): ListeningEventDao
}
```

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/ListeningEventStoreImpl.kt`

```kotlin
@Singleton
class ListeningEventStoreImpl @Inject constructor(
    private val dao: ListeningEventDao,
) : ListeningEventStore {

    override suspend fun saveEvent(event: ListeningEvent) {
        dao.insert(event.toEntity())
    }

    override suspend fun updateEvent(event: ListeningEvent) {
        dao.update(event.toEntity())
    }

    override fun getPendingSyncEvents(): Flow<List<ListeningEvent>> =
        dao.getPendingSync().map { entities ->
            entities.map { it.toDomain() }
        }

    override suspend fun updateSyncStatus(id: Long, status: SyncStatus) {
        dao.updateSyncStatus(id, status.name)
    }

    override suspend fun getActiveSession(trackId: String): ListeningEvent? =
        dao.getActiveSession(trackId)?.toDomain()

    override suspend fun deleteOldSyncedEvents(olderThanMs: Long) {
        dao.deleteOldSynced(olderThanMs)
    }

    private fun ListeningEvent.toEntity() = ListeningEventEntity(
        id = id,
        trackId = trackId,
        sessionId = sessionId,
        startedAt = startedAt,
        endedAt = endedAt,
        durationSeconds = durationSeconds,
        trackDurationSeconds = trackDurationSeconds,
        seekCount = seekCount,
        pauseCount = pauseCount,
        playbackContext = playbackContext.name,
        syncStatus = syncStatus.name,
        createdAt = createdAt,
    )

    private fun ListeningEventEntity.toDomain() = ListeningEvent(
        id = id,
        trackId = trackId,
        sessionId = sessionId,
        startedAt = startedAt,
        endedAt = endedAt,
        durationSeconds = durationSeconds,
        trackDurationSeconds = trackDurationSeconds,
        seekCount = seekCount,
        pauseCount = pauseCount,
        playbackContext = PlaybackContext.valueOf(playbackContext),
        syncStatus = SyncStatus.valueOf(syncStatus),
        createdAt = createdAt,
    )
}
```

---

## 3. Remote API Layer

### File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/model/ListeningEventRequest.kt`

```kotlin
@Serializable
data class ListeningEventRequest(
    @SerialName("track_id") val trackId: String,
    @SerialName("session_id") val sessionId: String? = null,
    @SerialName("started_at") val startedAt: Long? = null,
    @SerialName("ended_at") val endedAt: Long? = null,
    @SerialName("duration_seconds") val durationSeconds: Int,
    @SerialName("track_duration_seconds") val trackDurationSeconds: Int,
    @SerialName("seek_count") val seekCount: Int? = null,
    @SerialName("pause_count") val pauseCount: Int? = null,
    @SerialName("playback_context") val playbackContext: String? = null,
    @SerialName("client_type") val clientType: String? = null,
)

@Serializable
data class ListeningEventResponse(
    val id: Long,
    val created: Boolean,
)
```

### Update: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/RetrofitApiClient.kt`

Add to interface:
```kotlin
@POST("/v1/user/listening")
suspend fun recordListeningEvent(
    @Body request: ListeningEventRequest
): Response<ListeningEventResponse>
```

### Update: `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/RemoteApiClient.kt`

Add to interface:
```kotlin
suspend fun recordListeningEvent(request: ListeningEventRequest): RemoteApiResponse<ListeningEventResponse>
```

### Update: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/RemoteApiClientImpl.kt`

Add implementation:
```kotlin
override suspend fun recordListeningEvent(
    request: ListeningEventRequest
): RemoteApiResponse<ListeningEventResponse> = safeApiCall {
    retrofitApiClient.recordListeningEvent(request)
}
```

---

## 4. Dependency Injection

### Update: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`

```kotlin
@Provides
@Singleton
fun providesListeningEventDb(
    @ApplicationContext context: Context
): ListeningEventDb = Room.databaseBuilder(
    context,
    ListeningEventDb::class.java,
    "listening_event.db"
).build()

@Provides
@Singleton
fun providesListeningEventDao(db: ListeningEventDb): ListeningEventDao =
    db.listeningEventDao()

@Binds
@Singleton
abstract fun bindsListeningEventStore(
    impl: ListeningEventStoreImpl
): ListeningEventStore
```

### Update: `domain/src/main/java/com/lelloman/pezzottify/android/domain/app/AppInitializersModule.kt`

```kotlin
@Binds
@IntoSet
internal abstract fun bindsListeningTracker(
    tracker: ListeningTracker
): AppInitializer

@Binds
@IntoSet
internal abstract fun bindsListeningEventSynchronizer(
    synchronizer: ListeningEventSynchronizer
): AppInitializer
```

---

## 5. Implementation Sequence

### Phase 1: Data Layer
1. Create `ListeningEvent` domain model
2. Create `ListeningEventStore` interface
3. Create `ListeningEventEntity` Room entity
4. Create `ListeningEventDao` with queries
5. Create `ListeningEventDb` database
6. Implement `ListeningEventStoreImpl`
7. Add DI bindings in `LocalDataModule`

### Phase 2: Remote API
8. Create `ListeningEventRequest` and `ListeningEventResponse`
9. Add endpoint to `RetrofitApiClient`
10. Add method to `RemoteApiClient` interface
11. Implement in `RemoteApiClientImpl`

### Phase 3: Synchronization
12. Create `ListeningEventSynchronizer`
13. Register in `AppInitializersModule`

### Phase 4: Player Integration
14. Create `ListeningTracker`
15. Register in `AppInitializersModule`
16. Test playback event capture

### Phase 5: Testing
17. Unit tests for `ListeningTracker` logic
18. Unit tests for `ListeningEventSynchronizer`
19. Integration tests (requires test server)

---

## 6. Key Design Decisions

### Session ID Generation
- Generate UUID v4 when track starts playing
- Same session ID used for entire playback of one track
- Handles server deduplication for offline retry

### Minimum Duration Threshold
- Only report events with >= 5 seconds of actual listening
- Prevents spam from rapid track skipping

### Accumulated Duration
- Track actual listening time (pause time excluded)
- Update on pause/resume events

### Periodic Save
- Save progress every 30 seconds for long sessions
- Prevents data loss if app crashes

### Completion Calculation
- Server calculates completion (duration/trackDuration >= 90%)
- Client just reports raw duration

### Playback Context
- Capture where playback started (album, playlist, search, etc.)
- Passed from UI layer when initiating playback

### Sync Behavior
- New events saved with `PendingSync` status
- Synchronizer runs in background
- Network errors: retry with exponential backoff
- Client errors (4xx): mark as `SyncError`, don't retry
- Session ID deduplication makes retries safe

### Cleanup
- Delete synced events older than 30 days
- Run cleanup periodically (app startup)

---

## 7. Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `domain/.../listening/ListeningEvent.kt` | Create | Domain model |
| `domain/.../listening/ListeningEventStore.kt` | Create | Store interface |
| `domain/.../listening/ListeningTracker.kt` | Create | Playback monitor |
| `domain/.../listening/ListeningEventSynchronizer.kt` | Create | Background sync |
| `localdata/.../listening/model/ListeningEventEntity.kt` | Create | Room entity |
| `localdata/.../listening/ListeningEventDao.kt` | Create | Room DAO |
| `localdata/.../listening/ListeningEventDb.kt` | Create | Room database |
| `localdata/.../listening/ListeningEventStoreImpl.kt` | Create | Store impl |
| `localdata/.../listening/ListeningEventTypeConverters.kt` | Create | Type converters |
| `localdata/LocalDataModule.kt` | Modify | Add DI bindings |
| `remoteapi/.../model/ListeningEventRequest.kt` | Create | API models |
| `remoteapi/RetrofitApiClient.kt` | Modify | Add endpoint |
| `domain/.../remoteapi/RemoteApiClient.kt` | Modify | Add interface method |
| `remoteapi/RemoteApiClientImpl.kt` | Modify | Add implementation |
| `domain/.../app/AppInitializersModule.kt` | Modify | Register initializers |

---

## 8. Testing Strategy

### Unit Tests
- `ListeningTrackerTest`: Test state transitions, duration calculation, threshold filtering
- `ListeningEventSynchronizerTest`: Test sync logic, retry behavior, error handling
- `ListeningEventStoreImplTest`: Test entity mapping, queries

### Integration Tests
- Full flow: play track -> event saved -> sync to server
- Offline flow: play track -> event saved -> go offline -> come online -> sync
- Deduplication: same session ID synced twice -> server handles correctly

### Manual Testing
- Play track, verify event appears in server logs
- Kill app mid-playback, verify event still saved
- Go offline, play tracks, come online, verify sync
- Verify seek/pause counts accurate

---

## 9. Future Enhancements

- **Listening stats UI**: Show user their listening history/stats
- **Scrobbling**: Optional Last.fm integration
- **Smart recommendations**: Use listening data for personalized content
- **Playback analytics**: Admin dashboard for platform-wide stats
