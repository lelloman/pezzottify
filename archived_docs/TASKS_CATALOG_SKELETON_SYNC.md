# Catalog Skeleton Sync - Detailed Implementation Tasks

This document breaks down the implementation plan from `PLAN_CATALOG_SKELETON_SYNC.md` into small, actionable, sequential tasks.

**Status legend:**
- `[ ]` - Not started
- `[~]` - In progress
- `[x]` - Completed

---

## Phase 1: Server - Data Model & Storage

### 1.1 Add Skeleton Event Store Module

[x] **Task 1.1.1: Create skeleton event store module structure**

Create new module for skeleton event storage.

**Files to create:**
- `catalog-server/src/skeleton/mod.rs`
- `catalog-server/src/skeleton/schema.rs`
- `catalog-server/src/skeleton/store.rs`
- `catalog-server/src/skeleton/models.rs`

**Context:** Follow the pattern used in `server_store/` module. The schema will define two tables: `catalog_meta` and `catalog_events`.

```rust
// skeleton/mod.rs
pub mod models;
pub mod schema;
pub mod store;

pub use models::*;
pub use store::SkeletonEventStore;
```

---

[x] **Task 1.1.2: Define skeleton schema**

**File:** `catalog-server/src/skeleton/schema.rs`

**Context:** Use the `ServerSchema` pattern from `server_store/schema.rs`.

```rust
pub struct SkeletonSchema {
    pub version: usize,
    pub up: &'static str,
}

pub const SKELETON_VERSIONED_SCHEMAS: &[SkeletonSchema] = &[
    SkeletonSchema {
        version: 1,
        up: r#"
            CREATE TABLE catalog_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE catalog_events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                payload TEXT,
                timestamp INTEGER NOT NULL
            );

            CREATE INDEX idx_catalog_events_seq ON catalog_events(seq);
        "#,
    },
];
```

---

[x] **Task 1.1.3: Define skeleton event models**

**File:** `catalog-server/src/skeleton/models.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkeletonEventType {
    ArtistAdded,
    ArtistRemoved,
    AlbumAdded,
    AlbumRemoved,
    TrackAdded,
    TrackRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumAddedPayload {
    pub artist_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAddedPayload {
    pub album_id: String,
}

#[derive(Debug, Clone)]
pub struct SkeletonEvent {
    pub seq: i64,
    pub event_type: SkeletonEventType,
    pub entity_id: String,
    pub payload: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonChange {
    #[serde(rename = "type")]
    pub event_type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
}
```

---

[x] **Task 1.1.4: Implement SkeletonEventStore**

**File:** `catalog-server/src/skeleton/store.rs`

**Context:** Store manages catalog_meta and catalog_events tables. Uses shared connection from catalog store.

**Methods to implement:**
- `new(conn: Arc<Mutex<Connection>>) -> Result<Self>` - Initialize with shared connection
- `get_version() -> Result<i64>` - Get current catalog version from catalog_meta
- `get_checksum() -> Result<Option<String>>` - Get cached checksum
- `set_checksum(checksum: &str) -> Result<()>` - Update cached checksum
- `emit_event(event_type: SkeletonEventType, entity_id: &str, payload: Option<&str>) -> Result<i64>` - Insert event and increment version
- `get_events_since(seq: i64) -> Result<Vec<SkeletonEvent>>` - Get events after given sequence
- `get_earliest_seq() -> Result<i64>` - Get minimum seq (for 404 response)

```rust
pub struct SkeletonEventStore {
    conn: Arc<Mutex<Connection>>,
}

impl SkeletonEventStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        // Initialize schema if needed
        // ...
        Ok(Self { conn })
    }

    pub fn emit_event(
        &self,
        event_type: SkeletonEventType,
        entity_id: &str,
        payload: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO catalog_events (event_type, entity_id, payload, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![event_type.as_str(), entity_id, payload, timestamp],
        )?;

        // Increment version
        conn.execute(
            "INSERT OR REPLACE INTO catalog_meta (key, value)
             VALUES ('catalog_version', COALESCE(
                 (SELECT CAST(value AS INTEGER) + 1 FROM catalog_meta WHERE key = 'catalog_version'),
                 1
             ))",
            [],
        )?;

        let version: i64 = conn.query_row(
            "SELECT value FROM catalog_meta WHERE key = 'catalog_version'",
            [],
            |row| row.get(0),
        )?;

        Ok(version)
    }
    // ... other methods
}
```

---

[x] **Task 1.1.5: Add skeleton module to lib.rs**

**File:** `catalog-server/src/lib.rs`

Add the new module export:

```rust
pub mod skeleton;
```

---

### 1.2 Integrate Event Emission with Catalog Store

[x] **Task 1.2.1: Add SkeletonEventStore to SqliteCatalogStore**

**File:** `catalog-server/src/catalog_store/store.rs`

**Context:** Add skeleton event store as a field in SqliteCatalogStore. Initialize it in `new()`.

```rust
pub struct SqliteCatalogStore {
    conn: Arc<Mutex<Connection>>,
    media_base_path: PathBuf,
    changelog: ChangeLogStore,
    skeleton_events: SkeletonEventStore,  // Add this
}
```

Update `new()` to initialize skeleton event store after connection setup.

---

[x] **Task 1.2.2: Emit event on insert_artist**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~784)

**Context:** After successful artist insert, emit `artist_added` event.

```rust
pub fn insert_artist(&self, artist: &Artist) -> Result<()> {
    // ... existing validation and insert code ...

    // After successful insert:
    self.skeleton_events.emit_event(
        SkeletonEventType::ArtistAdded,
        &artist.id,
        None,
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.3: Emit event on insert_album**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~823)

**Context:** After successful album insert, emit `album_added` event with artist_ids payload.

```rust
pub fn insert_album(&self, album: &Album) -> Result<()> {
    // ... existing validation and insert code ...

    // After successful insert:
    let payload = AlbumAddedPayload {
        artist_ids: album.artist_ids.clone(),
    };
    self.skeleton_events.emit_event(
        SkeletonEventType::AlbumAdded,
        &album.id,
        Some(&serde_json::to_string(&payload)?),
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.4: Emit event on insert_track**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~871)

**Context:** After successful track insert, emit `track_added` event with album_id payload.

```rust
pub fn insert_track(&self, track: &Track) -> Result<()> {
    // ... existing validation and insert code ...

    // After successful insert:
    let payload = TrackAddedPayload {
        album_id: track.album_id.clone(),
    };
    self.skeleton_events.emit_event(
        SkeletonEventType::TrackAdded,
        &track.id,
        Some(&serde_json::to_string(&payload)?),
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.5: Emit event on delete_artist_record**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~1436)

```rust
pub fn delete_artist_record(&self, id: &str) -> Result<()> {
    // ... existing delete code ...

    // After successful delete:
    self.skeleton_events.emit_event(
        SkeletonEventType::ArtistRemoved,
        id,
        None,
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.6: Emit event on delete_album_record**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~1475)

```rust
pub fn delete_album_record(&self, id: &str) -> Result<()> {
    // ... existing delete code ...

    self.skeleton_events.emit_event(
        SkeletonEventType::AlbumRemoved,
        id,
        None,
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.7: Emit event on delete_track_record**

**File:** `catalog-server/src/catalog_store/store.rs` (line ~1514)

```rust
pub fn delete_track_record(&self, id: &str) -> Result<()> {
    // ... existing delete code ...

    self.skeleton_events.emit_event(
        SkeletonEventType::TrackRemoved,
        id,
        None,
    )?;

    Ok(())
}
```

---

[x] **Task 1.2.8: Expose skeleton_events getter on SqliteCatalogStore**

**File:** `catalog-server/src/catalog_store/store.rs`

Add getter method:

```rust
pub fn skeleton_events(&self) -> &SkeletonEventStore {
    &self.skeleton_events
}
```

---

### 1.3 Implement Checksum Calculation

[x] **Task 1.3.1: Add checksum calculation method**

**File:** `catalog-server/src/skeleton/store.rs`

**Context:** Calculate SHA256 of sorted IDs. Cache result in catalog_meta.

```rust
impl SkeletonEventStore {
    pub fn calculate_checksum(&self, catalog: &SqliteCatalogStore) -> Result<String> {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();

        // Get all artist IDs sorted
        let artist_ids = catalog.get_all_artist_ids()?;
        for id in &artist_ids {
            hasher.update(id.as_bytes());
        }

        // Get all album IDs sorted
        let album_ids = catalog.get_all_album_ids()?;
        for id in &album_ids {
            hasher.update(id.as_bytes());
        }

        // Get all track IDs sorted
        let track_ids = catalog.get_all_track_ids()?;
        for id in &track_ids {
            hasher.update(id.as_bytes());
        }

        let result = hasher.finalize();
        let checksum = format!("sha256:{}", hex::encode(result));

        // Cache the checksum
        self.set_checksum(&checksum)?;

        Ok(checksum)
    }
}
```

---

[x] **Task 1.3.2: Add helper methods to get all IDs from catalog**

**File:** `catalog-server/src/catalog_store/store.rs`

Add methods to retrieve sorted ID lists:

```rust
pub fn get_all_artist_ids(&self) -> Result<Vec<String>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id FROM artists ORDER BY id")?;
    let ids = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn get_all_album_ids(&self) -> Result<Vec<String>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id FROM albums ORDER BY id")?;
    let ids = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn get_all_track_ids(&self) -> Result<Vec<String>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id FROM tracks ORDER BY id")?;
    let ids = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}
```

---

[x] **Task 1.3.3: Add sha2 and hex dependencies**

**File:** `catalog-server/Cargo.toml`

```toml
[dependencies]
sha2 = "0.10"
hex = "0.4"
```

---

## Phase 2: Server - API Endpoints

### 2.1 Define API Response Types

[x] **Task 2.1.1: Create skeleton API response models**

**File:** `catalog-server/src/skeleton/models.rs` (append to existing)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonVersionResponse {
    pub version: i64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSkeletonResponse {
    pub version: i64,
    pub checksum: String,
    pub artists: Vec<String>,
    pub albums: Vec<SkeletonAlbumEntry>,
    pub tracks: Vec<SkeletonTrackEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonAlbumEntry {
    pub id: String,
    pub artist_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonTrackEntry {
    pub id: String,
    pub album_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonDeltaResponse {
    pub from_version: i64,
    pub to_version: i64,
    pub checksum: String,
    pub changes: Vec<SkeletonChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTooOldError {
    pub error: String,
    pub message: String,
    pub earliest_available: i64,
    pub current_version: i64,
}
```

---

### 2.2 Implement Skeleton Endpoints

[x] **Task 2.2.1: Create skeleton routes module**

**File:** `catalog-server/src/server/skeleton.rs`

**Context:** Create route handlers for skeleton endpoints. Follow pattern of existing routes in `server/` module.

```rust
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::skeleton::*;
use crate::server::state::AppState;

#[derive(Deserialize)]
pub struct DeltaQuery {
    since: i64,
}

pub async fn get_skeleton_version(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Get version and checksum from skeleton store
    // Return SkeletonVersionResponse
}

pub async fn get_full_skeleton(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Build full skeleton from catalog
    // Return FullSkeletonResponse
}

pub async fn get_skeleton_delta(
    State(state): State<AppState>,
    Query(params): Query<DeltaQuery>,
) -> impl IntoResponse {
    // Get events since version
    // If version too old, return 404 with VersionTooOldError
    // Otherwise return SkeletonDeltaResponse
}
```

---

[x] **Task 2.2.2: Implement get_skeleton_version handler**

**File:** `catalog-server/src/server/skeleton.rs`

```rust
pub async fn get_skeleton_version(
    State(state): State<AppState>,
) -> Result<Json<SkeletonVersionResponse>, StatusCode> {
    let catalog = state.catalog_store();
    let skeleton = catalog.skeleton_events();

    let version = skeleton.get_version()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get or calculate checksum
    let checksum = match skeleton.get_checksum() {
        Ok(Some(c)) => c,
        _ => skeleton.calculate_checksum(catalog)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    };

    Ok(Json(SkeletonVersionResponse { version, checksum }))
}
```

---

[x] **Task 2.2.3: Implement get_full_skeleton handler**

**File:** `catalog-server/src/server/skeleton.rs`

```rust
pub async fn get_full_skeleton(
    State(state): State<AppState>,
) -> Result<Json<FullSkeletonResponse>, StatusCode> {
    let catalog = state.catalog_store();
    let skeleton = catalog.skeleton_events();

    let version = skeleton.get_version()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let checksum = skeleton.calculate_checksum(catalog)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all artists (just IDs)
    let artists = catalog.get_all_artist_ids()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all albums with artist_ids
    let albums = catalog.get_all_albums_skeleton()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all tracks with album_id
    let tracks = catalog.get_all_tracks_skeleton()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(FullSkeletonResponse {
        version,
        checksum,
        artists,
        albums,
        tracks,
    }))
}
```

---

[x] **Task 2.2.4: Add skeleton query methods to catalog store**

**File:** `catalog-server/src/catalog_store/store.rs`

```rust
pub fn get_all_albums_skeleton(&self) -> Result<Vec<SkeletonAlbumEntry>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, artist_ids FROM albums ORDER BY id"
    )?;

    let albums = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let artist_ids_json: String = row.get(1)?;
        let artist_ids: Vec<String> = serde_json::from_str(&artist_ids_json)
            .unwrap_or_default();
        Ok(SkeletonAlbumEntry { id, artist_ids })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(albums)
}

pub fn get_all_tracks_skeleton(&self) -> Result<Vec<SkeletonTrackEntry>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, album_id FROM tracks ORDER BY id"
    )?;

    let tracks = stmt.query_map([], |row| {
        Ok(SkeletonTrackEntry {
            id: row.get(0)?,
            album_id: row.get(1)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(tracks)
}
```

---

[x] **Task 2.2.5: Implement get_skeleton_delta handler**

**File:** `catalog-server/src/server/skeleton.rs`

```rust
pub async fn get_skeleton_delta(
    State(state): State<AppState>,
    Query(params): Query<DeltaQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<VersionTooOldError>)> {
    let catalog = state.catalog_store();
    let skeleton = catalog.skeleton_events();

    let current_version = skeleton.get_version()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, /* ... */))?;

    let earliest = skeleton.get_earliest_seq()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, /* ... */))?;

    // Check if requested version is too old
    if params.since < earliest {
        return Err((
            StatusCode::NOT_FOUND,
            Json(VersionTooOldError {
                error: "version_too_old".to_string(),
                message: format!(
                    "Version {} is no longer available. Earliest available: {}",
                    params.since, earliest
                ),
                earliest_available: earliest,
                current_version,
            }),
        ));
    }

    // Get events since requested version
    let events = skeleton.get_events_since(params.since)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, /* ... */))?;

    // Convert events to changes
    let changes: Vec<SkeletonChange> = events.into_iter().map(|e| {
        let (artist_ids, album_id) = match e.event_type {
            SkeletonEventType::AlbumAdded => {
                let payload: Option<AlbumAddedPayload> = e.payload
                    .and_then(|p| serde_json::from_str(&p).ok());
                (payload.map(|p| p.artist_ids), None)
            }
            SkeletonEventType::TrackAdded => {
                let payload: Option<TrackAddedPayload> = e.payload
                    .and_then(|p| serde_json::from_str(&p).ok());
                (None, payload.map(|p| p.album_id))
            }
            _ => (None, None),
        };

        SkeletonChange {
            event_type: e.event_type.as_str().to_string(),
            id: e.entity_id,
            artist_ids,
            album_id,
        }
    }).collect();

    let checksum = skeleton.get_checksum()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, /* ... */))?
        .unwrap_or_else(|| {
            skeleton.calculate_checksum(catalog).unwrap_or_default()
        });

    Ok(Json(SkeletonDeltaResponse {
        from_version: params.since,
        to_version: current_version,
        checksum,
        changes,
    }))
}
```

---

[x] **Task 2.2.6: Register skeleton routes**

**File:** `catalog-server/src/server/server.rs`

Add routes under `/v1/catalog/skeleton`:

```rust
// In router setup:
.route("/v1/catalog/skeleton", get(skeleton::get_full_skeleton))
.route("/v1/catalog/skeleton/version", get(skeleton::get_skeleton_version))
.route("/v1/catalog/skeleton/delta", get(skeleton::get_skeleton_delta))
```

---

[x] **Task 2.2.7: Add skeleton module to server/mod.rs**

**File:** `catalog-server/src/server/mod.rs`

```rust
pub mod skeleton;
```

---

### 2.3 Write Server Tests

[x] **Task 2.3.1: Write unit tests for SkeletonEventStore**

**File:** `catalog-server/src/skeleton/store.rs` (add tests module)

Test cases:
- `test_emit_event_increments_version`
- `test_get_events_since_returns_correct_events`
- `test_checksum_calculation`
- `test_get_earliest_seq`

---

[x] **Task 2.3.2: Write integration tests for skeleton endpoints**

**File:** `catalog-server/tests/skeleton_api_tests.rs`

Test cases:
- `test_get_skeleton_version`
- `test_get_full_skeleton`
- `test_get_delta_returns_changes`
- `test_get_delta_returns_404_for_old_version`

---

## Phase 3: Android - Local Storage

### 3.1 Create Skeleton Database

[x] **Task 3.1.1: Create skeleton entity classes**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/skeleton/model/SkeletonEntities.kt`

```kotlin
package com.lelloman.pezzottify.android.localdata.internal.skeleton.model

import androidx.room.*

@Entity(tableName = "skeleton_artists")
data class SkeletonArtist(
    @PrimaryKey val id: String
)

@Entity(tableName = "skeleton_albums")
data class SkeletonAlbum(
    @PrimaryKey val id: String
)

@Entity(
    tableName = "skeleton_album_artists",
    primaryKeys = ["album_id", "artist_id"],
    foreignKeys = [
        ForeignKey(
            entity = SkeletonAlbum::class,
            parentColumns = ["id"],
            childColumns = ["album_id"],
            onDelete = ForeignKey.CASCADE
        ),
        ForeignKey(
            entity = SkeletonArtist::class,
            parentColumns = ["id"],
            childColumns = ["artist_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("artist_id")]
)
data class SkeletonAlbumArtist(
    @ColumnInfo(name = "album_id") val albumId: String,
    @ColumnInfo(name = "artist_id") val artistId: String
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

---

[x] **Task 3.1.2: Create SkeletonDao interface**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/skeleton/SkeletonDao.kt`

```kotlin
package com.lelloman.pezzottify.android.localdata.internal.skeleton

import androidx.room.*
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.*

@Dao
interface SkeletonDao {
    // Queries
    @Query("SELECT value FROM skeleton_meta WHERE key = 'version'")
    suspend fun getVersion(): String?

    @Query("SELECT value FROM skeleton_meta WHERE key = 'checksum'")
    suspend fun getChecksum(): String?

    @Query("SELECT album_id FROM skeleton_album_artists WHERE artist_id = :artistId")
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    @Query("SELECT id FROM skeleton_tracks WHERE album_id = :albumId")
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    @Query("SELECT COUNT(*) FROM skeleton_artists")
    suspend fun getArtistCount(): Int

    @Query("SELECT COUNT(*) FROM skeleton_albums")
    suspend fun getAlbumCount(): Int

    @Query("SELECT COUNT(*) FROM skeleton_tracks")
    suspend fun getTrackCount(): Int

    // Full sync operations
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtists(artists: List<SkeletonArtist>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbums(albums: List<SkeletonAlbum>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbumArtists(albumArtists: List<SkeletonAlbumArtist>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTracks(tracks: List<SkeletonTrack>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun setMeta(meta: SkeletonMeta)

    @Query("DELETE FROM skeleton_tracks")
    suspend fun deleteAllTracks()

    @Query("DELETE FROM skeleton_album_artists")
    suspend fun deleteAllAlbumArtists()

    @Query("DELETE FROM skeleton_albums")
    suspend fun deleteAllAlbums()

    @Query("DELETE FROM skeleton_artists")
    suspend fun deleteAllArtists()

    @Transaction
    suspend fun replaceAll(
        artists: List<SkeletonArtist>,
        albums: List<SkeletonAlbum>,
        albumArtists: List<SkeletonAlbumArtist>,
        tracks: List<SkeletonTrack>,
        version: String,
        checksum: String
    ) {
        deleteAllTracks()
        deleteAllAlbumArtists()
        deleteAllAlbums()
        deleteAllArtists()
        insertArtists(artists)
        insertAlbums(albums)
        insertAlbumArtists(albumArtists)
        insertTracks(tracks)
        setMeta(SkeletonMeta("version", version))
        setMeta(SkeletonMeta("checksum", checksum))
    }

    // Delta operations
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: SkeletonArtist)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: SkeletonAlbum)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbumArtist(albumArtist: SkeletonAlbumArtist)

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

[x] **Task 3.1.3: Create SkeletonDb Room database**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/skeleton/SkeletonDb.kt`

```kotlin
package com.lelloman.pezzottify.android.localdata.internal.skeleton

import androidx.room.Database
import androidx.room.RoomDatabase
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.*

@Database(
    entities = [
        SkeletonArtist::class,
        SkeletonAlbum::class,
        SkeletonAlbumArtist::class,
        SkeletonTrack::class,
        SkeletonMeta::class
    ],
    version = 1,
    exportSchema = true
)
internal abstract class SkeletonDb : RoomDatabase() {
    abstract fun skeletonDao(): SkeletonDao

    companion object {
        const val NAME = "SkeletonDb"
    }
}
```

---

[x] **Task 3.1.4: Add SkeletonDb to DbModule**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/DbModule.kt`

Add Room database and DAO providers:

```kotlin
@Provides
@Singleton
fun provideSkeletonDb(
    @ApplicationContext context: Context
): SkeletonDb = Room.databaseBuilder(
    context,
    SkeletonDb::class.java,
    SkeletonDb.NAME
).build()

@Provides
@Singleton
fun provideSkeletonDao(db: SkeletonDb): SkeletonDao = db.skeletonDao()
```

---

### 3.2 Create Domain Interface

[x] **Task 3.2.1: Create SkeletonStore interface in domain**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/skeleton/SkeletonStore.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.skeleton

interface SkeletonStore {
    suspend fun getVersion(): Long?
    suspend fun getChecksum(): String?
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    suspend fun replaceAll(
        artists: List<String>,
        albums: List<SkeletonAlbumData>,
        tracks: List<SkeletonTrackData>,
        version: Long,
        checksum: String
    )

    suspend fun applyDelta(
        changes: List<SkeletonChange>,
        version: Long,
        checksum: String
    )

    suspend fun clear()
}

data class SkeletonAlbumData(
    val id: String,
    val artistIds: List<String>
)

data class SkeletonTrackData(
    val id: String,
    val albumId: String
)

data class SkeletonChange(
    val type: String,
    val id: String,
    val artistIds: List<String>? = null,
    val albumId: String? = null
)
```

---

[x] **Task 3.2.2: Implement SkeletonStoreImpl**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/skeleton/SkeletonStoreImpl.kt`

```kotlin
package com.lelloman.pezzottify.android.localdata.internal.skeleton

import com.lelloman.pezzottify.android.domain.skeleton.*
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.*
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SkeletonStoreImpl @Inject constructor(
    private val db: SkeletonDb,
    private val dao: SkeletonDao
) : SkeletonStore {

    override suspend fun getVersion(): Long? =
        dao.getVersion()?.toLongOrNull()

    override suspend fun getChecksum(): String? =
        dao.getChecksum()

    override suspend fun getAlbumIdsForArtist(artistId: String): List<String> =
        dao.getAlbumIdsForArtist(artistId)

    override suspend fun getTrackIdsForAlbum(albumId: String): List<String> =
        dao.getTrackIdsForAlbum(albumId)

    override suspend fun replaceAll(
        artists: List<String>,
        albums: List<SkeletonAlbumData>,
        tracks: List<SkeletonTrackData>,
        version: Long,
        checksum: String
    ) {
        val albumArtists = albums.flatMap { album ->
            album.artistIds.map { artistId ->
                SkeletonAlbumArtist(album.id, artistId)
            }
        }

        dao.replaceAll(
            artists = artists.map { SkeletonArtist(it) },
            albums = albums.map { SkeletonAlbum(it.id) },
            albumArtists = albumArtists,
            tracks = tracks.map { SkeletonTrack(it.id, it.albumId) },
            version = version.toString(),
            checksum = checksum
        )
    }

    override suspend fun applyDelta(
        changes: List<SkeletonChange>,
        version: Long,
        checksum: String
    ) {
        db.withTransaction {
            for (change in changes) {
                when (change.type) {
                    "artist_added" -> dao.insertArtist(SkeletonArtist(change.id))
                    "album_added" -> {
                        dao.insertAlbum(SkeletonAlbum(change.id))
                        change.artistIds?.forEach { artistId ->
                            dao.insertAlbumArtist(SkeletonAlbumArtist(change.id, artistId))
                        }
                    }
                    "track_added" -> {
                        change.albumId?.let { albumId ->
                            dao.insertTrack(SkeletonTrack(change.id, albumId))
                        }
                    }
                    "artist_removed" -> dao.deleteArtist(change.id)
                    "album_removed" -> dao.deleteAlbum(change.id)
                    "track_removed" -> dao.deleteTrack(change.id)
                }
            }
            dao.setMeta(SkeletonMeta("version", version.toString()))
            dao.setMeta(SkeletonMeta("checksum", checksum))
        }
    }

    override suspend fun clear() {
        dao.deleteAllTracks()
        dao.deleteAllAlbumArtists()
        dao.deleteAllAlbums()
        dao.deleteAllArtists()
    }
}
```

---

[x] **Task 3.2.3: Add SkeletonStore binding to LocalDataModule**

**File:** `android/localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`

```kotlin
@Binds
abstract fun bindSkeletonStore(impl: SkeletonStoreImpl): SkeletonStore
```

---

## Phase 4: Android - Sync Logic

### 4.1 Add API Endpoints

[x] **Task 4.1.1: Add skeleton response DTOs**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/response/SkeletonResponses.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class SkeletonVersionResponse(
    val version: Long,
    val checksum: String
)

@Serializable
data class FullSkeletonResponse(
    val version: Long,
    val checksum: String,
    val artists: List<String>,
    val albums: List<SkeletonAlbumDto>,
    val tracks: List<SkeletonTrackDto>
)

@Serializable
data class SkeletonAlbumDto(
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>
)

@Serializable
data class SkeletonTrackDto(
    val id: String,
    @SerialName("album_id") val albumId: String
)

@Serializable
data class SkeletonDeltaResponse(
    @SerialName("from_version") val fromVersion: Long,
    @SerialName("to_version") val toVersion: Long,
    val checksum: String,
    val changes: List<SkeletonChangeDto>
)

@Serializable
data class SkeletonChangeDto(
    val type: String,
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>? = null,
    @SerialName("album_id") val albumId: String? = null
)
```

---

[x] **Task 4.1.2: Add skeleton methods to RemoteApiClient interface**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/RemoteApiClient.kt`

Add to interface:

```kotlin
suspend fun getSkeletonVersion(): RemoteApiResponse<SkeletonVersionResponse>

suspend fun getFullSkeleton(): RemoteApiResponse<FullSkeletonResponse>

suspend fun getSkeletonDelta(sinceVersion: Long): RemoteApiResponse<SkeletonDeltaResponse>
```

---

[x] **Task 4.1.3: Add skeleton endpoints to RetrofitApiClient**

**File:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RetrofiApiClient.kt`

```kotlin
@GET("v1/catalog/skeleton/version")
suspend fun getSkeletonVersion(): Response<SkeletonVersionResponse>

@GET("v1/catalog/skeleton")
suspend fun getFullSkeleton(): Response<FullSkeletonResponse>

@GET("v1/catalog/skeleton/delta")
suspend fun getSkeletonDelta(@Query("since") sinceVersion: Long): Response<SkeletonDeltaResponse>
```

---

[x] **Task 4.1.4: Implement skeleton methods in RemoteApiClientImpl**

**File:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt`

```kotlin
override suspend fun getSkeletonVersion(): RemoteApiResponse<SkeletonVersionResponse> =
    safeApiCall { api.getSkeletonVersion() }

override suspend fun getFullSkeleton(): RemoteApiResponse<FullSkeletonResponse> =
    safeApiCall { api.getFullSkeleton() }

override suspend fun getSkeletonDelta(sinceVersion: Long): RemoteApiResponse<SkeletonDeltaResponse> =
    safeApiCall { api.getSkeletonDelta(sinceVersion) }
```

---

### 4.2 Implement Syncer

[x] **Task 4.2.1: Create CatalogSkeletonSyncer**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/skeleton/CatalogSkeletonSyncer.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.skeleton

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class CatalogSkeletonSyncer @Inject constructor(
    private val api: RemoteApiClient,
    private val skeletonStore: SkeletonStore,
    private val logger: Logger
) {
    sealed class SyncResult {
        object Success : SyncResult()
        object AlreadyUpToDate : SyncResult()
        data class Failed(val error: String) : SyncResult()
    }

    suspend fun sync(): SyncResult {
        val localVersion = skeletonStore.getVersion() ?: 0L

        if (localVersion == 0L) {
            logger.i(TAG, "No local version, performing full sync")
            return fullSync()
        }

        logger.i(TAG, "Local version $localVersion, attempting delta sync")

        return when (val response = api.getSkeletonDelta(localVersion)) {
            is RemoteApiResponse.Success -> {
                if (response.data.changes.isEmpty()) {
                    logger.i(TAG, "Already up to date at version ${response.data.toVersion}")
                    SyncResult.AlreadyUpToDate
                } else {
                    applyDelta(response.data)
                }
            }
            is RemoteApiResponse.Error.NotFound -> {
                logger.w(TAG, "Version $localVersion too old, performing full sync")
                fullSync()
            }
            is RemoteApiResponse.Error -> {
                logger.e(TAG, "Delta sync failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    suspend fun forceFullSync(): SyncResult = fullSync()

    suspend fun verifyChecksum(): Boolean {
        val localChecksum = skeletonStore.getChecksum() ?: return false

        return when (val response = api.getSkeletonVersion()) {
            is RemoteApiResponse.Success -> {
                val match = response.data.checksum == localChecksum
                if (!match) {
                    logger.w(TAG, "Checksum mismatch! Local: $localChecksum, Remote: ${response.data.checksum}")
                }
                match
            }
            else -> {
                logger.e(TAG, "Failed to verify checksum: $response")
                false
            }
        }
    }

    private suspend fun fullSync(): SyncResult {
        return when (val response = api.getFullSkeleton()) {
            is RemoteApiResponse.Success -> {
                val data = response.data
                logger.i(TAG, "Full sync received - ${data.artists.size} artists, ${data.albums.size} albums, ${data.tracks.size} tracks")

                skeletonStore.replaceAll(
                    artists = data.artists,
                    albums = data.albums.map { SkeletonAlbumData(it.id, it.artistIds) },
                    tracks = data.tracks.map { SkeletonTrackData(it.id, it.albumId) },
                    version = data.version,
                    checksum = data.checksum
                )

                logger.i(TAG, "Full sync complete at version ${data.version}")
                SyncResult.Success
            }
            is RemoteApiResponse.Error -> {
                logger.e(TAG, "Full sync failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    private suspend fun applyDelta(delta: SkeletonDeltaResponse): SyncResult {
        logger.i(TAG, "Applying ${delta.changes.size} changes (${delta.fromVersion} -> ${delta.toVersion})")

        val changes = delta.changes.map { dto ->
            SkeletonChange(
                type = dto.type,
                id = dto.id,
                artistIds = dto.artistIds,
                albumId = dto.albumId
            )
        }

        skeletonStore.applyDelta(changes, delta.toVersion, delta.checksum)

        logger.i(TAG, "Delta applied, now at version ${delta.toVersion}")
        return SyncResult.Success
    }

    companion object {
        private const val TAG = "CatalogSkeletonSyncer"
    }
}
```

---

### 4.3 Integrate with App Lifecycle

[x] **Task 4.3.1: Add skeleton sync to app initialization**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/usecase/InitializeApp.kt`

Add skeleton sync call during app initialization (after authentication check):

```kotlin
// After confirming user is logged in:
launch { skeletonSyncer.sync() }
```

---

[x] **Task 4.3.2: Add skeleton sync to SyncManager (if exists) or create sync trigger points**

**Context:** Trigger skeleton sync on:
- App start (already done in InitializeApp)
- Pull-to-refresh in library
- Periodic background job (optional)

---

## Phase 5: Android - Use Skeleton for Discography

### 5.1 Create Discography Provider

[x] **Task 5.1.1: Create DiscographyProvider**

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/statics/DiscographyProvider.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DiscographyProvider @Inject constructor(
    private val skeletonStore: SkeletonStore
) {
    /**
     * Get album IDs for an artist from the local skeleton.
     * Always returns current data (no cache staleness).
     */
    suspend fun getAlbumIdsForArtist(artistId: String): List<String> {
        return skeletonStore.getAlbumIdsForArtist(artistId)
    }
}
```

---

[x] **Task 5.1.2: Update ArtistScreen ViewModel to use DiscographyProvider**

**File:** `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/artist/ArtistViewModel.kt`

Replace discography fetching logic:

```kotlin
// Old approach:
// val discography = staticsCache.getDiscography(artistId) ?: api.getArtistDiscography(artistId)

// New approach:
val albumIds = discographyProvider.getAlbumIdsForArtist(artistId)
// Then fetch album details for each ID (can still use cache for album details)
```

---

## Phase 6: Testing & Verification

### 6.1 Server Tests

[x] **Task 6.1.1: Verify event emission with catalog operations**

Manual test: Insert artist/album/track via admin API, verify events are logged in catalog_events table.

---

[x] **Task 6.1.2: Test full skeleton endpoint with sample data**

Test that GET /v1/catalog/skeleton returns correct structure and data.

---

[x] **Task 6.1.3: Test delta endpoint with various scenarios**

- Request delta from version 0 (should get all events)
- Request delta from recent version (should get subset)
- Request delta from version that doesn't exist (should get 404)

---

### 6.2 Android Tests

[x] **Task 6.2.1: Write unit tests for SkeletonStoreImpl**

**File:** `android/localdata/src/test/java/com/lelloman/pezzottify/android/localdata/internal/skeleton/SkeletonStoreImplTest.kt`

---

[x] **Task 6.2.2: Write unit tests for CatalogSkeletonSyncer**

**File:** `android/domain/src/test/java/com/lelloman/pezzottify/android/domain/skeleton/CatalogSkeletonSyncerTest.kt`

---

[x] **Task 6.2.3: Integration test skeleton sync flow**

End-to-end test with catalog-server: sync, verify data, modify catalog, sync again, verify delta applied.

---

## Summary

| Phase | Tasks | Status |
|-------|-------|--------|
| 1. Server Data Model | 1.1.1 - 1.3.3 | 11/11 |
| 2. Server Endpoints | 2.1.1 - 2.3.2 | 9/9 |
| 3. Android Storage | 3.1.1 - 3.2.3 | 7/7 |
| 4. Android Sync | 4.1.1 - 4.3.2 | 7/7 |
| 5. Use Skeleton | 5.1.1 - 5.1.2 | 2/2 |
| 6. Testing | 6.1.1 - 6.2.3 | 6/6 |
| **Total** | | **42/42** |
