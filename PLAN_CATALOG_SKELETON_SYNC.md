# Catalog Skeleton Sync - Implementation Plan

## Overview

Sync the catalog structure (IDs + relationships) to client devices, treating it like user data. Always incremental, with checksum verification and manual full-sync escape hatch.

**Problem solved:** Client caches artist discographies, but when new albums are added, the cache becomes stale. Instead of complex cache invalidation, we sync the entire catalog "skeleton" (just IDs and relationships) to the device.

**Key insight:** The skeleton is small (~20MB for 2M items compressed), append-mostly, and changes infrequently. Perfect for incremental sync with long-lasting cursors.

---

## Phase 1: Server - Data Model & Storage

### 1.1 Catalog Version Tracking

Add to `server_store` (or new table in catalog DB):

```sql
CREATE TABLE catalog_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Stores:
--   catalog_version (integer, increments on each change)
--   catalog_checksum (hash of all IDs for verification)
```

### 1.2 Catalog Event Log

New table for append-only catalog changes:

```sql
CREATE TABLE catalog_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,  -- 'artist_added', 'album_added', 'track_added', 'artist_removed', etc.
    entity_id TEXT NOT NULL,
    payload TEXT,              -- JSON: {"artist_ids": [...]} for albums, {"album_id": "..."} for tracks
    timestamp INTEGER NOT NULL
);
CREATE INDEX idx_catalog_events_seq ON catalog_events(seq);
```

**Event types:**
- `artist_added` - payload: `null`
- `album_added` - payload: `{"artist_ids": ["id1", "id2"]}`
- `track_added` - payload: `{"album_id": "album123"}`
- `artist_removed` - payload: `null` (rare)
- `album_removed` - payload: `null` (rare)
- `track_removed` - payload: `null` (rare)

### 1.3 Emit Events on Catalog Changes

Modify catalog store write operations to emit events:

- `insert_artist` → emit `artist_added`
- `insert_album` → emit `album_added` with `artist_ids`
- `insert_track` → emit `track_added` with `album_id`
- `delete_artist` → emit `artist_removed`
- `delete_album` → emit `album_removed`
- `delete_track` → emit `track_removed`

After each event, increment `catalog_version` in `catalog_meta`.

**Checksum calculation:**
- Compute on-demand or cache in `catalog_meta`
- Algorithm: `SHA256(sorted(all_artist_ids) + sorted(all_album_ids) + sorted(all_track_ids))`
- Update checksum lazily (e.g., on version endpoint hit if stale)

---

## Phase 2: Server - API Endpoints

### 2.1 Full Skeleton

**`GET /v1/catalog/skeleton`**

Returns the complete catalog skeleton. Used for initial sync or forced resync.

```json
{
  "version": 12345,
  "checksum": "sha256:abc123def456...",
  "artists": ["artist_id_1", "artist_id_2", "..."],
  "albums": [
    {"id": "album_1", "artist_ids": ["artist_id_1"]},
    {"id": "album_2", "artist_ids": ["artist_id_1", "artist_id_2"]}
  ],
  "tracks": [
    {"id": "track_1", "album_id": "album_1"},
    {"id": "track_2", "album_id": "album_1"},
    {"id": "track_3", "album_id": "album_2"}
  ]
}
```

**Response headers:**
- `Content-Encoding: gzip` (compressed)
- `Cache-Control: no-cache` (always fresh)

### 2.2 Version Check

**`GET /v1/catalog/skeleton/version`**

Lightweight endpoint for quick version/checksum verification.

```json
{
  "version": 12345,
  "checksum": "sha256:abc123def456..."
}
```

### 2.3 Delta Sync

**`GET /v1/catalog/skeleton/delta?since={version}`**

Returns changes since the given version.

**Success response (200):**
```json
{
  "from_version": 12340,
  "to_version": 12345,
  "changes": [
    {"type": "artist_added", "id": "new_artist_1"},
    {"type": "album_added", "id": "new_album_1", "artist_ids": ["new_artist_1"]},
    {"type": "track_added", "id": "new_track_1", "album_id": "new_album_1"},
    {"type": "track_added", "id": "new_track_2", "album_id": "new_album_1"}
  ]
}
```

**Version too old response (410 Gone):**
```json
{
  "error": "version_too_old",
  "message": "Version 100 is no longer available. Earliest available: 5000",
  "earliest_available": 5000,
  "current_version": 12345
}
```

Client should perform full sync when receiving 410.

**Note:** For now, never prune catalog events. They're small (append-only IDs) and we want cursors to last forever.

---

## Phase 3: Android - Local Storage

### 3.1 New Room Database

Create a separate database for skeleton data (or add to existing statics DB):

```kotlin
@Database(
    entities = [
        SkeletonArtist::class,
        SkeletonAlbum::class,
        SkeletonTrack::class,
        SkeletonMeta::class
    ],
    version = 1
)
abstract class SkeletonDatabase : RoomDatabase() {
    abstract fun skeletonDao(): SkeletonDao
}
```

### 3.2 Entities

```kotlin
@Entity(tableName = "skeleton_artists")
data class SkeletonArtist(
    @PrimaryKey val id: String
)

@Entity(tableName = "skeleton_albums")
data class SkeletonAlbum(
    @PrimaryKey val id: String,
    @ColumnInfo(name = "artist_ids")
    val artistIds: String  // JSON array: ["id1", "id2"]
)

@Entity(
    tableName = "skeleton_tracks",
    foreignKeys = [ForeignKey(
        entity = SkeletonAlbum::class,
        parentColumns = ["id"],
        childColumns = ["album_id"],
        onDelete = ForeignKey.CASCADE
    )],
    indices = [Index("album_id")]
)
data class SkeletonTrack(
    @PrimaryKey val id: String,
    @ColumnInfo(name = "album_id") val albumId: String
)

@Entity(tableName = "skeleton_meta")
data class SkeletonMeta(
    @PrimaryKey val key: String,
    val value: String
)
```

### 3.3 DAO

```kotlin
@Dao
interface SkeletonDao {
    // === Queries ===

    @Query("SELECT value FROM skeleton_meta WHERE key = 'version'")
    suspend fun getVersion(): String?

    @Query("SELECT value FROM skeleton_meta WHERE key = 'checksum'")
    suspend fun getChecksum(): String?

    @Query("SELECT id FROM skeleton_albums WHERE artist_ids LIKE '%' || :artistId || '%'")
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    @Query("SELECT id FROM skeleton_tracks WHERE album_id = :albumId")
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    @Query("SELECT COUNT(*) FROM skeleton_artists")
    suspend fun getArtistCount(): Int

    @Query("SELECT COUNT(*) FROM skeleton_albums")
    suspend fun getAlbumCount(): Int

    @Query("SELECT COUNT(*) FROM skeleton_tracks")
    suspend fun getTrackCount(): Int

    // === Full sync operations ===

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtists(artists: List<SkeletonArtist>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbums(albums: List<SkeletonAlbum>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTracks(tracks: List<SkeletonTrack>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun setMeta(meta: SkeletonMeta)

    @Query("DELETE FROM skeleton_artists")
    suspend fun deleteAllArtists()

    @Query("DELETE FROM skeleton_albums")
    suspend fun deleteAllAlbums()

    @Query("DELETE FROM skeleton_tracks")
    suspend fun deleteAllTracks()

    @Transaction
    suspend fun replaceAll(
        artists: List<SkeletonArtist>,
        albums: List<SkeletonAlbum>,
        tracks: List<SkeletonTrack>,
        version: String,
        checksum: String
    ) {
        deleteAllTracks()
        deleteAllAlbums()
        deleteAllArtists()
        insertArtists(artists)
        insertAlbums(albums)
        insertTracks(tracks)
        setMeta(SkeletonMeta("version", version))
        setMeta(SkeletonMeta("checksum", checksum))
    }

    // === Delta operations ===

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: SkeletonArtist)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: SkeletonAlbum)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTrack(track: SkeletonTrack)

    @Query("DELETE FROM skeleton_artists WHERE id = :id")
    suspend fun deleteArtist(id: String)

    @Query("DELETE FROM skeleton_albums WHERE id = :id")
    suspend fun deleteAlbum(id: String)

    @Query("DELETE FROM skeleton_tracks WHERE id = :id")
    suspend fun deleteTrack(id: String)
}
```

---

## Phase 4: Android - Sync Logic

### 4.1 API Interface

Add to `RemoteApiClient`:

```kotlin
interface RemoteApiClient {
    // ... existing methods ...

    suspend fun getSkeletonVersion(): RemoteApiResponse<SkeletonVersionResponse>

    suspend fun getFullSkeleton(): RemoteApiResponse<FullSkeletonResponse>

    suspend fun getSkeletonDelta(sinceVersion: Long): RemoteApiResponse<SkeletonDeltaResponse>
}

data class SkeletonVersionResponse(
    val version: Long,
    val checksum: String
)

data class FullSkeletonResponse(
    val version: Long,
    val checksum: String,
    val artists: List<String>,
    val albums: List<SkeletonAlbumDto>,
    val tracks: List<SkeletonTrackDto>
)

data class SkeletonAlbumDto(
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>
)

data class SkeletonTrackDto(
    val id: String,
    @SerialName("album_id") val albumId: String
)

data class SkeletonDeltaResponse(
    @SerialName("from_version") val fromVersion: Long,
    @SerialName("to_version") val toVersion: Long,
    val changes: List<SkeletonChange>
)

data class SkeletonChange(
    val type: String,  // "artist_added", "album_added", "track_added", "*_removed"
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>? = null,
    @SerialName("album_id") val albumId: String? = null
)
```

### 4.2 Syncer Implementation

```kotlin
class CatalogSkeletonSyncer @Inject constructor(
    private val api: RemoteApiClient,
    private val skeletonDao: SkeletonDao,
    private val logger: Logger
) {
    sealed class SyncResult {
        object Success : SyncResult()
        object AlreadyUpToDate : SyncResult()
        data class Failed(val error: String) : SyncResult()
    }

    /**
     * Main sync entry point. Attempts delta sync, falls back to full sync if needed.
     */
    suspend fun sync(): SyncResult {
        val localVersion = skeletonDao.getVersion()?.toLongOrNull() ?: 0L

        if (localVersion == 0L) {
            logger.i("Skeleton: No local version, performing full sync")
            return fullSync()
        }

        logger.i("Skeleton: Local version $localVersion, attempting delta sync")

        return when (val response = api.getSkeletonDelta(localVersion)) {
            is RemoteApiResponse.Success -> {
                if (response.data.changes.isEmpty()) {
                    logger.i("Skeleton: Already up to date at version ${response.data.toVersion}")
                    SyncResult.AlreadyUpToDate
                } else {
                    applyDelta(response.data)
                }
            }
            is RemoteApiResponse.Error.NotFound -> {
                // 410 Gone - version too old, need full sync
                logger.w("Skeleton: Version $localVersion too old, performing full sync")
                fullSync()
            }
            is RemoteApiResponse.Error -> {
                logger.e("Skeleton: Delta sync failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    /**
     * Force a full resync, ignoring local state.
     */
    suspend fun forceFullSync(): SyncResult = fullSync()

    /**
     * Verify local checksum matches server. Returns true if in sync.
     */
    suspend fun verifyChecksum(): Boolean {
        val localChecksum = skeletonDao.getChecksum() ?: return false

        return when (val response = api.getSkeletonVersion()) {
            is RemoteApiResponse.Success -> {
                val match = response.data.checksum == localChecksum
                if (!match) {
                    logger.w("Skeleton: Checksum mismatch! Local: $localChecksum, Remote: ${response.data.checksum}")
                }
                match
            }
            else -> {
                logger.e("Skeleton: Failed to verify checksum: $response")
                false
            }
        }
    }

    private suspend fun fullSync(): SyncResult {
        return when (val response = api.getFullSkeleton()) {
            is RemoteApiResponse.Success -> {
                val data = response.data
                logger.i("Skeleton: Full sync received - ${data.artists.size} artists, ${data.albums.size} albums, ${data.tracks.size} tracks")

                skeletonDao.replaceAll(
                    artists = data.artists.map { SkeletonArtist(it) },
                    albums = data.albums.map { SkeletonAlbum(it.id, Json.encodeToString(it.artistIds)) },
                    tracks = data.tracks.map { SkeletonTrack(it.id, it.albumId) },
                    version = data.version.toString(),
                    checksum = data.checksum
                )

                logger.i("Skeleton: Full sync complete at version ${data.version}")
                SyncResult.Success
            }
            is RemoteApiResponse.Error -> {
                logger.e("Skeleton: Full sync failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    private suspend fun applyDelta(delta: SkeletonDeltaResponse): SyncResult {
        logger.i("Skeleton: Applying ${delta.changes.size} changes (${delta.fromVersion} -> ${delta.toVersion})")

        for (change in delta.changes) {
            when (change.type) {
                "artist_added" -> skeletonDao.insertArtist(SkeletonArtist(change.id))
                "album_added" -> {
                    val artistIds = change.artistIds ?: emptyList()
                    skeletonDao.insertAlbum(SkeletonAlbum(change.id, Json.encodeToString(artistIds)))
                }
                "track_added" -> {
                    val albumId = change.albumId ?: continue
                    skeletonDao.insertTrack(SkeletonTrack(change.id, albumId))
                }
                "artist_removed" -> skeletonDao.deleteArtist(change.id)
                "album_removed" -> skeletonDao.deleteAlbum(change.id)
                "track_removed" -> skeletonDao.deleteTrack(change.id)
            }
        }

        skeletonDao.setMeta(SkeletonMeta("version", delta.toVersion.toString()))
        // Note: checksum not updated on delta - verify periodically

        logger.i("Skeleton: Delta applied, now at version ${delta.toVersion}")
        return SyncResult.Success
    }
}
```

### 4.3 Sync Triggers

```kotlin
class SyncOrchestrator @Inject constructor(
    private val skeletonSyncer: CatalogSkeletonSyncer,
    private val userDataSyncer: UserDataSyncer,  // existing
    private val logger: Logger
) {
    /**
     * Called on app start. Syncs in background, non-blocking.
     */
    suspend fun onAppStart() {
        coroutineScope {
            launch { skeletonSyncer.sync() }
            launch { userDataSyncer.sync() }
        }
    }

    /**
     * Called on pull-to-refresh in library.
     */
    suspend fun onLibraryRefresh() {
        skeletonSyncer.sync()
    }

    /**
     * Called periodically (e.g., WorkManager weekly job).
     */
    suspend fun periodicIntegrityCheck() {
        if (!skeletonSyncer.verifyChecksum()) {
            logger.w("Skeleton checksum mismatch, forcing full sync")
            skeletonSyncer.forceFullSync()
        }
    }

    /**
     * Called from Settings when user taps "Resync catalog".
     */
    suspend fun forceFullResync() {
        skeletonSyncer.forceFullSync()
    }
}
```

---

## Phase 5: Android - Use Skeleton for Discography

### 5.1 New Discography Provider

Replace the current cached discography approach with skeleton-based lookup:

```kotlin
class DiscographyProvider @Inject constructor(
    private val skeletonDao: SkeletonDao,
    private val albumRepository: AlbumRepository  // For fetching full album details
) {
    /**
     * Get album IDs for an artist from the local skeleton.
     * Always returns current data (no cache staleness).
     */
    suspend fun getAlbumIdsForArtist(artistId: String): List<String> {
        return skeletonDao.getAlbumIdsForArtist(artistId)
    }

    /**
     * Get full album details for an artist.
     * Uses skeleton for IDs, then fetches/caches album details.
     */
    suspend fun getDiscography(artistId: String): List<Album> {
        val albumIds = skeletonDao.getAlbumIdsForArtist(artistId)
        return albumIds.mapNotNull { albumId ->
            albumRepository.getAlbum(albumId)  // This can still use cache for album details
        }
    }
}
```

### 5.2 Update ArtistScreen ViewModel

```kotlin
// Old approach (remove):
// val discography = staticsCache.getDiscography(artistId) ?: api.getDiscography(artistId)

// New approach:
val albumIds = discographyProvider.getAlbumIdsForArtist(artistId)
val albums = albumIds.map { albumRepository.getAlbum(it) }
```

### 5.3 Benefits

- **Discography is always up-to-date** - skeleton is synced
- **Album details still cached** - no change to album caching
- **No stale discography problem** - the original issue is solved!
- **Offline browsing works** - skeleton is local, album cache is local

---

## Phase 6: Migration & Rollout

### 6.1 Server-Side (Backwards Compatible)

1. Add `catalog_meta` and `catalog_events` tables
2. Implement event emission in catalog store operations
3. Add skeleton endpoints (new, doesn't break existing clients)
4. Deploy and verify events are being logged

### 6.2 Android-Side (Feature Flagged)

1. Add skeleton database and DAO
2. Add skeleton sync logic
3. Add API methods for skeleton endpoints
4. Add feature flag: `use_skeleton_for_discography`
5. When flag ON: use `DiscographyProvider`
6. When flag OFF: use existing cache approach

### 6.3 Testing

1. Test full sync with various catalog sizes
2. Test delta sync with additions/removals
3. Test checksum verification
4. Test recovery from corrupted state
5. Test offline behavior
6. Performance test with 2M items

### 6.4 Rollout

1. Enable for internal testing (flag ON for test users)
2. Monitor sync times, storage usage, error rates
3. Gradual rollout to all users
4. Remove feature flag, remove old discography cache code

---

## Open Questions

### Q1: Include names in skeleton?

**Option A: IDs only (current plan)**
- Smaller payload (~20MB for 2M items)
- Must fetch details to display anything

**Option B: Include names**
- Larger payload (+30 bytes/item = +60MB)
- Enables offline search
- Can show artist/album names without network

**Recommendation:** Start with IDs only. Add names later if offline search is needed.

### Q2: Include image IDs in skeleton?

**Option A: No images**
- Smaller payload

**Option B: Include display_image_id**
- Can show thumbnails without fetching full details
- +22 bytes per album/artist

**Recommendation:** Add later if needed for performance.

### Q3: Event pruning strategy?

**Option A: Never prune**
- Simplest, cursors last forever
- Events are small (just IDs)
- Estimate: 1M events × 50 bytes = 50MB total

**Option B: Prune after N months**
- Keeps DB smaller
- Risk: old clients need full sync

**Recommendation:** Never prune initially. Revisit if DB grows too large.

### Q4: Compression?

**Wire:** Gzip (automatic with most HTTP clients)
**Storage:** Uncompressed (SQLite default)

**Recommendation:** Keep it simple. SQLite storage is fine uncompressed.

---

## Estimated Effort

| Phase | Description | Effort |
|-------|-------------|--------|
| 1 | Server data model & storage | 2-3 hours |
| 2 | Server API endpoints | 2-3 hours |
| 3 | Android local storage | 2-3 hours |
| 4 | Android sync logic | 3-4 hours |
| 5 | Use skeleton for discography | 2-3 hours |
| 6 | Migration & rollout | 1-2 hours |
| | **Total** | **12-18 hours** |

---

## Summary

This approach treats the catalog structure as a synced dataset, just like user data. The client always knows what exists in the catalog (from the skeleton), and fetches details on demand (with caching).

**Key benefits:**
- Cache invalidation problem is gone
- Simple mental model
- Offline browsing of catalog structure
- Incremental sync is efficient
- Self-healing with checksum verification

**Trade-offs:**
- Initial sync downloads ~20MB
- Additional storage on device (~100MB worst case)
- New sync infrastructure to maintain

The trade-offs are minimal compared to the complexity of real-time cache invalidation via sync events.
