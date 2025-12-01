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

Following existing patterns from `UserContentSynchronizer` and `BaseSynchronizer`:

```
Player Events (PlayerImpl)
    ↓
ListeningTracker (monitors playback state via combined flows)
    ↓
ListeningEventStore.saveEvent(syncStatus=PendingSync)
    ↓ (Room Database - UserContentDb)
ListeningEventEntity
    ↓
ListeningEventSynchronizer (extends BaseSynchronizer)
    ↓
RemoteApiClient.recordListeningEvent(ListeningEventSyncData)
    ↓
Server API
```

---

## 1. Domain Layer

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEvent.kt`

```kotlin
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import java.util.UUID

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
    val playbackContext: PlaybackPlaylistContext,
    val syncStatus: SyncStatus = SyncStatus.PendingSync,
    val createdAt: Long,
)

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

fun PlaybackPlaylistContext.toApiString(): String = when (this) {
    is PlaybackPlaylistContext.Album -> "album"  // Album context only exists when unmodified
    is PlaybackPlaylistContext.UserPlaylist -> if (isEdited) "queue" else "playlist"
    is PlaybackPlaylistContext.UserMix -> "queue"
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventSyncData.kt`

```kotlin
/**
 * Domain DTO for syncing listening events to the server.
 * This keeps the domain layer independent of remoteapi models.
 */
data class ListeningEventSyncData(
    val trackId: String,
    val sessionId: String,
    val startedAt: Long,        // Unix timestamp seconds
    val endedAt: Long?,         // Unix timestamp seconds
    val durationSeconds: Int,
    val trackDurationSeconds: Int,
    val seekCount: Int,
    val pauseCount: Int,
    val playbackContext: String,
)
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventStore.kt`

```kotlin
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

interface ListeningEventStore {
    /** Saves event and returns the generated ID */
    suspend fun saveEvent(event: ListeningEvent): Long

    suspend fun updateEvent(event: ListeningEvent)

    suspend fun getPendingSyncEvents(): List<ListeningEvent>

    suspend fun updateSyncStatus(id: Long, status: SyncStatus)

    suspend fun getActiveSession(trackId: String): ListeningEvent?

    suspend fun deleteEvent(id: Long)

    suspend fun deleteOldNonSyncedEvents(olderThanMs: Long): Int

    suspend fun deleteAll()
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt`

```kotlin
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Tracks playback state and generates listening events.
 *
 * Session lifecycle:
 * - Start: When a new track begins playing
 * - Continue: While track plays (accumulating duration, counting pauses/seeks)
 * - Periodic save: Every 10 seconds, update DB record with current progress
 * - End: When track changes, playback stops, or after 5 minutes of inactivity
 */
@Singleton
class ListeningTracker @Inject constructor(
    private val player: PezzottifyPlayer,
    private val listeningEventStore: ListeningEventStore,
    private val timeProvider: TimeProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(ListeningTracker::class)
    private var currentSession: ActiveSession? = null
    private var lastTrackIndex: Int? = null

    data class ActiveSession(
        val sessionId: String,
        val trackId: String,
        val trackDurationSeconds: Int,
        val startedAt: Long,
        var accumulatedDurationMs: Long = 0,
        var lastResumeTime: Long? = null,
        var lastPauseTime: Long? = null,
        var seekCount: Int = 0,
        var pauseCount: Int = 0,
        val playbackContext: PlaybackPlaylistContext,
        var savedEventId: Long? = null,  // DB record ID for updates
    )

    private data class PlaybackState(
        val trackIndex: Int?,
        val isPlaying: Boolean,
    )

    override fun initialize() {
        // Monitor track changes and play state together to avoid race conditions
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

        // Monitor seek events separately
        scope.launch {
            player.seekEvents.collect {
                onSeekEvent()
            }
        }

        // Periodic save loop
        scope.launch {
            while (true) {
                delay(PERIODIC_SAVE_INTERVAL_MS)
                saveCurrentSessionProgress()
            }
        }
    }

    private suspend fun handlePlaybackStateChange(state: PlaybackState) {
        if (state.trackIndex != lastTrackIndex) {
            // Track changed - finalize previous session and start new one
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

    private suspend fun startNewSession(trackIndex: Int, isPlaying: Boolean) {
        val playlist = player.playbackPlaylist.value ?: return
        val trackId = playlist.tracksIds.getOrNull(trackIndex) ?: return

        // Get track duration from player (populated when track loads)
        val trackDurationSeconds = player.currentTrackDurationSeconds.value ?: 0

        currentSession = ActiveSession(
            sessionId = UUID.randomUUID().toString(),
            trackId = trackId,
            trackDurationSeconds = trackDurationSeconds,
            startedAt = timeProvider.nowUtcMs(),
            lastResumeTime = if (isPlaying) timeProvider.nowUtcMs() else null,
            playbackContext = playlist.context,
        )

        logger.d("Started new listening session for track $trackId")
    }

    private suspend fun onPlayStateChanged(isPlaying: Boolean) {
        val session = currentSession ?: return

        if (isPlaying) {
            // Check for inactivity timeout on resume
            if (shouldStartNewSessionOnResume(session)) {
                finalizeSession(session)
                lastTrackIndex?.let { startNewSession(it, true) }
                return
            }
            session.lastResumeTime = timeProvider.nowUtcMs()
            session.lastPauseTime = null
        } else {
            // Accumulate duration on pause
            session.lastResumeTime?.let { resumeTime ->
                session.accumulatedDurationMs += timeProvider.nowUtcMs() - resumeTime
                session.lastResumeTime = null
                session.lastPauseTime = timeProvider.nowUtcMs()
                session.pauseCount++
            }
        }
    }

    private fun shouldStartNewSessionOnResume(session: ActiveSession): Boolean {
        val pauseTime = session.lastPauseTime ?: return false
        val pauseDuration = timeProvider.nowUtcMs() - pauseTime
        return pauseDuration > INACTIVITY_TIMEOUT_MS
    }

    private fun onSeekEvent() {
        currentSession?.let { it.seekCount++ }
    }

    private suspend fun saveCurrentSessionProgress() {
        val session = currentSession ?: return
        val duration = calculateFinalDuration(session)

        if (duration < MIN_DURATION_THRESHOLD_SEC) return

        val event = createEventFromSession(session, endedAt = null)

        if (session.savedEventId == null) {
            // First save - insert new record
            session.savedEventId = listeningEventStore.saveEvent(event)
            logger.d("Saved session progress ${session.sessionId}, duration: ${duration}s")
        } else {
            // Update existing record
            listeningEventStore.updateEvent(event.copy(id = session.savedEventId!!))
            logger.d("Updated session progress ${session.sessionId}, duration: ${duration}s")
        }
    }

    private suspend fun finalizeSession(session: ActiveSession) {
        val finalDuration = calculateFinalDuration(session)

        // Only save if minimum threshold met (5 seconds)
        if (finalDuration >= MIN_DURATION_THRESHOLD_SEC) {
            val event = createEventFromSession(session, endedAt = timeProvider.nowUtcMs())

            if (session.savedEventId == null) {
                listeningEventStore.saveEvent(event)
            } else {
                listeningEventStore.updateEvent(event.copy(id = session.savedEventId!!))
            }
            logger.d("Finalized listening session ${session.sessionId}, duration: ${finalDuration}s")
        } else {
            // Delete any saved progress if below threshold
            session.savedEventId?.let { listeningEventStore.deleteEvent(it) }
            logger.d("Session ${session.sessionId} discarded, duration ${finalDuration}s below threshold")
        }

        currentSession = null
    }

    private fun createEventFromSession(session: ActiveSession, endedAt: Long?): ListeningEvent {
        return ListeningEvent(
            trackId = session.trackId,
            sessionId = session.sessionId,
            startedAt = session.startedAt,
            endedAt = endedAt,
            durationSeconds = calculateFinalDuration(session),
            trackDurationSeconds = session.trackDurationSeconds,
            seekCount = session.seekCount,
            pauseCount = session.pauseCount,
            playbackContext = session.playbackContext,
            createdAt = session.startedAt,  // Use session start time
        )
    }

    private fun calculateFinalDuration(session: ActiveSession): Int {
        var totalMs = session.accumulatedDurationMs

        // Add current playing segment if still playing
        session.lastResumeTime?.let { resumeTime ->
            totalMs += timeProvider.nowUtcMs() - resumeTime
        }

        return (totalMs / 1000).toInt()
    }

    companion object {
        const val MIN_DURATION_THRESHOLD_SEC = 5
        const val PERIODIC_SAVE_INTERVAL_MS = 10_000L  // 10 seconds
        const val INACTIVITY_TIMEOUT_MS = 300_000L    // 5 minutes
    }
}
```

### File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventSynchronizer.kt`

```kotlin
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Background synchronizer that sends pending listening events to server.
 *
 * Extends BaseSynchronizer for consistent behavior with other synchronizers:
 * - Exponential backoff on failures
 * - Sleep/wake mechanism
 * - Session ID deduplication handles retries
 */
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
        val deleted = listeningEventStore.deleteOldNonSyncedEvents(cutoff)
        if (deleted > 0) {
            logger.i("Cleaned up $deleted old non-synced listening events")
        }
    }

    private suspend fun syncEvent(event: ListeningEvent) {
        listeningEventStore.updateSyncStatus(event.id, SyncStatus.Syncing)

        val result = remoteApiClient.recordListeningEvent(event.toSyncData())

        when (result) {
            is RemoteApiResponse.Success -> {
                // Delete immediately after successful sync
                listeningEventStore.deleteEvent(event.id)
                logger.d("Successfully synced listening event ${event.sessionId}")
            }
            is RemoteApiResponse.Error.Network -> {
                // Retry later
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.d("Network error syncing event ${event.sessionId}, will retry")
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                // Retry later (user might log back in)
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.d("Unauthorized syncing event ${event.sessionId}, will retry")
            }
            else -> {
                // Retry infinitely (conform to existing pattern)
                listeningEventStore.updateSyncStatus(event.id, SyncStatus.PendingSync)
                logger.e("Failed to sync listening event ${event.sessionId}: $result")
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

## 2. Player Interface Updates

### Update: `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/ControlsAndStatePlayer.kt`

Add seek events flow and current track duration:
```kotlin
interface ControlsAndStatePlayer {
    // ... existing members ...

    /**
     * Duration of the currently playing track in seconds.
     * Populated when track info is loaded/fetched.
     * Used by ListeningTracker for listening stats.
     */
    val currentTrackDurationSeconds: StateFlow<Int?>

    /**
     * Emits when a seek operation occurs (seekToPercentage, forward10Sec, rewind10Sec).
     * Used by ListeningTracker to count seeks.
     */
    val seekEvents: SharedFlow<SeekEvent>

    data class SeekEvent(
        val timestamp: Long,
    )
}
```

### Update: `player/src/main/java/com/lelloman/pezzottify/android/player/PlayerImpl.kt`

Add current track duration and seek event emission:
```kotlin
// Track duration - populated when track loads
private val _currentTrackDurationSeconds = MutableStateFlow<Int?>(null)
override val currentTrackDurationSeconds: StateFlow<Int?> = _currentTrackDurationSeconds.asStateFlow()

// Seek events
private val _seekEvents = MutableSharedFlow<SeekEvent>()
override val seekEvents: SharedFlow<SeekEvent> = _seekEvents

// In loadAlbum() or wherever track info is fetched:
// When track starts playing, fetch its info and set duration
scope.launch {
    val trackId = playlist.tracksIds[currentIndex]
    val track = staticsProvider.provideTrack(trackId)
        .filterIsInstance<StaticsItem.Loaded<Track>>()
        .first()
    _currentTrackDurationSeconds.value = track.data.durationSeconds
}

// Clear when playback stops
override fun stop() {
    _currentTrackDurationSeconds.value = null
    // ... rest of implementation
}

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

---

## 3. Local Data Layer

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/ListeningEventEntity.kt`

```kotlin
import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "listening_event",
    indices = [
        Index("sync_status"),
        Index("session_id", unique = true),
    ]
)
internal data class ListeningEventEntity(
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
import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update

@Dao
internal interface ListeningEventDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(event: ListeningEventEntity): Long

    @Update
    suspend fun update(event: ListeningEventEntity)

    @Query("SELECT * FROM listening_event WHERE sync_status = 'PendingSync' ORDER BY created_at ASC")
    suspend fun getPendingSync(): List<ListeningEventEntity>

    @Query("UPDATE listening_event SET sync_status = :status WHERE id = :id")
    suspend fun updateSyncStatus(id: Long, status: String)

    @Query("SELECT * FROM listening_event WHERE track_id = :trackId AND ended_at IS NULL LIMIT 1")
    suspend fun getActiveSession(trackId: String): ListeningEventEntity?

    @Query("DELETE FROM listening_event WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM listening_event WHERE sync_status != 'Synced' AND created_at < :olderThanMs")
    suspend fun deleteOldNonSynced(olderThanMs: Long): Int

    @Query("DELETE FROM listening_event")
    suspend fun deleteAll()
}
```

### Update: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/usercontent/UserContentDb.kt`

Add ListeningEventEntity to existing database:
```kotlin
@Database(
    entities = [LikedContentEntity::class, ListeningEventEntity::class],
    version = 2,  // Bump version
    exportSchema = true,
)
internal abstract class UserContentDb : RoomDatabase() {
    abstract fun likedContentDao(): LikedContentDao
    abstract fun listeningEventDao(): ListeningEventDao
}
```

Add migration:
```kotlin
val MIGRATION_1_2 = object : Migration(1, 2) {
    override fun migrate(database: SupportSQLiteDatabase) {
        database.execSQL("""
            CREATE TABLE IF NOT EXISTS listening_event (
                id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                track_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                duration_seconds INTEGER NOT NULL,
                track_duration_seconds INTEGER NOT NULL,
                seek_count INTEGER NOT NULL,
                pause_count INTEGER NOT NULL,
                playback_context TEXT NOT NULL,
                sync_status TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )
        """)
        database.execSQL("CREATE INDEX IF NOT EXISTS index_listening_event_sync_status ON listening_event (sync_status)")
        database.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS index_listening_event_session_id ON listening_event (session_id)")
    }
}
```

### File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/listening/ListeningEventStoreImpl.kt`

```kotlin
import com.lelloman.pezzottify.android.domain.listening.ListeningEvent
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class ListeningEventStoreImpl @Inject constructor(
    private val dao: ListeningEventDao,
) : ListeningEventStore {

    override suspend fun saveEvent(event: ListeningEvent): Long {
        return dao.insert(event.toEntity())
    }

    override suspend fun updateEvent(event: ListeningEvent) {
        dao.update(event.toEntity())
    }

    override suspend fun getPendingSyncEvents(): List<ListeningEvent> =
        dao.getPendingSync().map { it.toDomain() }

    override suspend fun updateSyncStatus(id: Long, status: SyncStatus) {
        dao.updateSyncStatus(id, status.name)
    }

    override suspend fun getActiveSession(trackId: String): ListeningEvent? =
        dao.getActiveSession(trackId)?.toDomain()

    override suspend fun deleteEvent(id: Long) {
        dao.delete(id)
    }

    override suspend fun deleteOldNonSyncedEvents(olderThanMs: Long): Int =
        dao.deleteOldNonSynced(olderThanMs)

    override suspend fun deleteAll() {
        dao.deleteAll()
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
        playbackContext = playbackContext.toStorageString(),
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
        playbackContext = playbackContext.toPlaybackContext(),
        syncStatus = SyncStatus.valueOf(syncStatus),
        createdAt = createdAt,
    )

    private fun PlaybackPlaylistContext.toStorageString(): String = when (this) {
        is PlaybackPlaylistContext.Album -> "Album:$albumId"
        is PlaybackPlaylistContext.UserPlaylist -> "UserPlaylist:$userPlaylistId"
        is PlaybackPlaylistContext.UserMix -> "UserMix"
    }

    private fun String.toPlaybackContext(): PlaybackPlaylistContext = when {
        startsWith("Album:") -> PlaybackPlaylistContext.Album(removePrefix("Album:"))
        startsWith("UserPlaylist:") -> PlaybackPlaylistContext.UserPlaylist(
            userPlaylistId = removePrefix("UserPlaylist:"),
            isEdited = false,
        )
        else -> PlaybackPlaylistContext.UserMix
    }
}
```

---

## 4. Remote API Layer

### File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/model/ListeningEventRequest.kt`

```kotlin
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ListeningEventRequest(
    @SerialName("track_id") val trackId: String,
    @SerialName("session_id") val sessionId: String,
    @SerialName("started_at") val startedAt: Long,
    @SerialName("ended_at") val endedAt: Long?,
    @SerialName("duration_seconds") val durationSeconds: Int,
    @SerialName("track_duration_seconds") val trackDurationSeconds: Int,
    @SerialName("seek_count") val seekCount: Int,
    @SerialName("pause_count") val pauseCount: Int,
    @SerialName("playback_context") val playbackContext: String,
    @SerialName("client_type") val clientType: String,
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
suspend fun recordListeningEvent(data: ListeningEventSyncData): RemoteApiResponse<ListeningEventResponse>
```

### Update: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/RemoteApiClientImpl.kt`

Add implementation:
```kotlin
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
```

---

## 5. Dependency Injection

### Update: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`

```kotlin
@Provides
@Singleton
internal fun provideListeningEventDao(db: UserContentDb): ListeningEventDao =
    db.listeningEventDao()

@Provides
@Singleton
internal fun provideListeningEventStore(dao: ListeningEventDao): ListeningEventStore =
    ListeningEventStoreImpl(dao)
```

Update UserContentDb provider to include migration:
```kotlin
@Provides
@Singleton
internal fun provideUserContentDb(
    @ApplicationContext context: Context
): UserContentDb = Room.databaseBuilder(
    context,
    UserContentDb::class.java,
    "user_content.db"
)
    .addMigrations(MIGRATION_1_2)
    .build()
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

### Update: Logout flow

In `PerformLogout` use case or equivalent:
```kotlin
// Clear listening events on logout
listeningEventStore.deleteAll()
```

---

## 6. Implementation Sequence

### Phase 1: Player Updates
1. Add `SeekEvent` data class and `seekEvents` flow to `ControlsAndStatePlayer` interface
2. Implement seek event emission in `PlayerImpl`

### Phase 2: Domain Layer
3. Create `ListeningEventSyncData` DTO
4. Create `ListeningEvent` domain model with `toSyncData()` extension
5. Create `ListeningEventStore` interface

### Phase 3: Local Data Layer
6. Create `ListeningEventEntity` Room entity
7. Create `ListeningEventDao` with queries
8. Add entity to `UserContentDb` with migration
9. Implement `ListeningEventStoreImpl`
10. Update `LocalDataModule` with DI bindings

### Phase 4: Remote API
11. Create `ListeningEventRequest` and `ListeningEventResponse`
12. Add endpoint to `RetrofitApiClient`
13. Add method to `RemoteApiClient` interface
14. Implement in `RemoteApiClientImpl`

### Phase 5: Synchronization
15. Create `ListeningEventSynchronizer` extending `BaseSynchronizer`
16. Register in `AppInitializersModule`

### Phase 6: Tracking
17. Create `ListeningTracker`
18. Register in `AppInitializersModule`

### Phase 7: Cleanup
19. Add `deleteAll()` call to logout flow

### Phase 8: Testing
20. Unit tests for `ListeningTracker` logic
21. Unit tests for `ListeningEventSynchronizer`
22. Integration tests (requires test server)

---

## 7. Key Design Decisions

### Session ID Generation
- Generate UUID v4 when track starts playing
- Same session ID used for entire playback of one track
- Handles server deduplication for offline retry

### Minimum Duration Threshold
- Only report events with >= 5 seconds of actual listening
- Prevents spam from rapid track skipping

### Accumulated Duration
- Track actual listening time (pause time excluded)
- Update on pause/resume events via combined flow

### Playback Context
- Uses existing `PlaybackPlaylistContext` sealed interface
- Maps to API strings: Album → "album", UserPlaylist → "playlist", UserMix → "queue"

### Sync Behavior
- Extends `BaseSynchronizer` for consistent behavior
- Events deleted immediately after successful sync
- All errors result in retry (PendingSync status)
- Exponential backoff between retries

### Cleanup
- Delete synced events immediately after sync
- Delete non-synced events older than 7 days on app startup (in `onBeforeMainLoop`)
- Delete all events on logout

### Database
- Events stored in `UserContentDb` alongside liked content
- Both are user content that syncs to server

### Race Condition Prevention
- Track index and play state combined into single flow
- Prevents incorrect duration calculation from out-of-order events

---

## 8. Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `domain/.../player/ControlsAndStatePlayer.kt` | Modify | Add SeekEvent and seekEvents flow |
| `player/.../PlayerImpl.kt` | Modify | Emit seek events |
| `domain/.../listening/ListeningEvent.kt` | Create | Domain model |
| `domain/.../listening/ListeningEventSyncData.kt` | Create | Sync DTO |
| `domain/.../listening/ListeningEventStore.kt` | Create | Store interface |
| `domain/.../listening/ListeningTracker.kt` | Create | Playback monitor |
| `domain/.../listening/ListeningEventSynchronizer.kt` | Create | Background sync |
| `localdata/.../listening/ListeningEventEntity.kt` | Create | Room entity |
| `localdata/.../listening/ListeningEventDao.kt` | Create | Room DAO |
| `localdata/.../usercontent/UserContentDb.kt` | Modify | Add entity + migration |
| `localdata/.../listening/ListeningEventStoreImpl.kt` | Create | Store impl |
| `localdata/LocalDataModule.kt` | Modify | Add DI bindings |
| `remoteapi/.../model/ListeningEventRequest.kt` | Create | API models |
| `remoteapi/RetrofitApiClient.kt` | Modify | Add endpoint |
| `domain/.../remoteapi/RemoteApiClient.kt` | Modify | Add interface method |
| `remoteapi/RemoteApiClientImpl.kt` | Modify | Add implementation |
| `domain/.../app/AppInitializersModule.kt` | Modify | Register initializers |
| `domain/.../usecase/PerformLogout.kt` | Modify | Clear events on logout |

---

## 9. Testing Strategy

### Unit Tests
- `ListeningTrackerTest`: Test state transitions, duration calculation, threshold filtering, seek counting
- `ListeningEventSynchronizerTest`: Test sync logic, retry behavior, cleanup
- `ListeningEventStoreImplTest`: Test entity mapping, queries

### Integration Tests
- Full flow: play track -> event saved -> sync to server
- Offline flow: play track -> event saved -> go offline -> come online -> sync
- Deduplication: same session ID synced twice -> server handles correctly
- Logout: verify all events deleted

### Manual Testing
- Play track, verify event appears in server logs
- Kill app mid-playback, verify event still saved (if above threshold)
- Go offline, play tracks, come online, verify sync
- Verify seek/pause counts accurate
- Verify logout clears events

---

## 10. Resolved Questions

All design questions have been resolved. See `LISTENING_STATS_PLAN_REVIEW.md` for detailed discussion.

| Question | Decision |
|----------|----------|
| Q1: AppInitializer scope | Keep constructor injection (no interface change needed) |
| Q2: playback_context purpose | Statistics only. Map: Album→"album", UserPlaylist→"playlist" (or "queue" if edited), UserMix→"queue" |
| Q3: Rename UserDataDb | Yes, rename to `UserLocalDataDb` |
| Q4: Track duration source | Add `currentTrackDurationSeconds: StateFlow<Int?>` to player interface |
| Q5: Session lifecycle | Keep sessions with periodic saves (10s), inactivity timeout (5min), savedEventId for updates |

---

## 11. Future Enhancements

- **Listening stats UI**: Show user their listening history/stats
- **Scrobbling**: Optional Last.fm integration
- **Smart recommendations**: Use listening data for personalized content
- **Playback analytics**: Admin dashboard for platform-wide stats
