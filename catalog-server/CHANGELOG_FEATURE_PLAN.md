# Catalog Change Log Implementation Plan

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Overall | ⏳ Not Started | Planning complete, implementation pending |

---

## Overview

Implement a catalog change log system that:

- Tracks field-level changes to artists, albums, tracks, and images
- Supports manual batches for grouping related changes
- Serves both user-facing "what's new" and admin audit purposes
- Requires an active batch before catalog modifications are allowed

## Design Decisions

| Decision                  | Choice                                                              |
| ------------------------- | ------------------------------------------------------------------- |
| Batch ID format           | UUID                                                                |
| Batch states              | Two states: Open → Closed (closing = published)                     |
| Concurrent batches        | One active batch at a time (409 Conflict if trying to create another) |
| Unbatched changes         | Not allowed - require active batch for modifications                |
| Field-level tracking      | JSON blob for diffs (matches existing patterns)                     |
| User attribution          | Not tracked                                                         |
| Entity snapshot           | Always populated (full JSON of entity after change)                 |
| Delete batch              | Only allowed if batch is empty (no changes recorded)                |
| Batch closing             | Explicit only (no auto-close)                                       |
| Stale batch alert         | Telegram alert after 1 hour of inactivity on open batch             |
| Display summary (add/del) | List individual names for artists/albums, counts for tracks         |
| Display summary (update)  | Counts only (e.g., "updated 3 artists")                             |

## Database Schema (Version 2)

### Table: `catalog_batches`

```sql
CREATE TABLE catalog_batches (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    is_open INTEGER NOT NULL DEFAULT 1,  -- 1 = open, 0 = closed
    created_at INTEGER NOT NULL,
    closed_at INTEGER,
    last_activity_at INTEGER NOT NULL    -- Updated on each change, used for stale batch alerts
);

CREATE INDEX idx_batches_is_open ON catalog_batches(is_open);
CREATE INDEX idx_batches_closed_at ON catalog_batches(closed_at DESC);
```

### Table: `catalog_change_log`

```sql
CREATE TABLE catalog_change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id TEXT NOT NULL REFERENCES catalog_batches(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,      -- 'artist', 'album', 'track', 'image'
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL,        -- 'create', 'update', 'delete'
    field_changes TEXT NOT NULL,    -- JSON: {"field": {"old": X, "new": Y}, ...}
    entity_snapshot TEXT NOT NULL,  -- Full entity JSON after change (before for deletes)
    display_summary TEXT,           -- Human-readable summary
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_changelog_batch ON catalog_change_log(batch_id);
CREATE INDEX idx_changelog_entity ON catalog_change_log(entity_type, entity_id);
CREATE INDEX idx_changelog_created ON catalog_change_log(created_at DESC);
```

## Rust Models

```rust
// In catalog_store/changelog.rs

pub enum ChangeOperation {
    Create,
    Update,
    Delete,
}

pub enum ChangeEntityType {
    Artist,
    Album,
    Track,
    Image,
}

pub struct CatalogBatch {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_open: bool,
    pub created_at: i64,
    pub closed_at: Option<i64>,
    pub last_activity_at: i64,
}

pub struct ChangeEntry {
    pub id: i64,
    pub batch_id: String,
    pub entity_type: ChangeEntityType,
    pub entity_id: String,
    pub operation: ChangeOperation,
    pub field_changes: serde_json::Value,
    pub entity_snapshot: serde_json::Value,  // Always populated
    pub display_summary: Option<String>,
    pub created_at: i64,
}

pub struct FieldChange {
    pub old: Option<serde_json::Value>,
    pub new: Option<serde_json::Value>,
}
```

## Architecture

### ChangeLogStore

A new `ChangeLogStore` struct that shares the same `Arc<Mutex<Connection>>` as `SqliteCatalogStore`:

```rust
pub struct ChangeLogStore {
    conn: Arc<Mutex<Connection>>,
}

impl ChangeLogStore {
    // Batch management
    pub fn create_batch(&self, name: &str, description: Option<&str>) -> Result<CatalogBatch>;
    pub fn get_batch(&self, id: &str) -> Result<Option<CatalogBatch>>;
    pub fn get_active_batch(&self) -> Result<Option<CatalogBatch>>;
    pub fn close_batch(&self, id: &str) -> Result<()>;
    pub fn list_batches(&self, is_open: Option<bool>) -> Result<Vec<CatalogBatch>>;

    // Change recording (called within existing transaction)
    pub fn record_change_internal(
        &self,
        conn: &MutexGuard<Connection>,  // Existing locked connection
        batch_id: &str,
        entity_type: ChangeEntityType,
        entity_id: &str,
        operation: ChangeOperation,
        before: Option<&serde_json::Value>,
        after: Option<&serde_json::Value>,
    ) -> Result<()>;

    // Query changes
    pub fn get_batch_changes(&self, batch_id: &str) -> Result<Vec<ChangeEntry>>;
    pub fn get_entity_history(&self, entity_type: ChangeEntityType, entity_id: &str) -> Result<Vec<ChangeEntry>>;
}
```

### Integration with SqliteCatalogStore

Modify `SqliteCatalogStore` to:

1. Hold a reference to `ChangeLogStore`
2. Check for active batch before write operations
3. Capture before state and record changes within the same transaction

```rust
pub struct SqliteCatalogStore {
    conn: Arc<Mutex<Connection>>,
    media_base_path: PathBuf,
    change_log: ChangeLogStore,  // NEW
}

// Example: update_artist with change logging
fn update_artist(&self, id: &str, data: serde_json::Value) -> Result<serde_json::Value> {
    let conn = self.conn.lock().unwrap();

    // 1. Check for active batch
    let active_batch = self.get_active_batch_internal(&conn)?
        .ok_or(CatalogError::NoBatchActive)?;

    // 2. Begin transaction
    conn.execute("BEGIN IMMEDIATE", [])?;

    // 3. Capture before state
    let before = self.get_artist_json_internal(&conn, id)?;

    // 4. Perform update
    // ... existing update logic ...

    // 5. Get after state
    let after = serde_json::to_value(&artist)?;

    // 6. Record change
    self.change_log.record_change_internal(
        &conn,
        &active_batch.id,
        ChangeEntityType::Artist,
        id,
        ChangeOperation::Update,
        before.as_ref(),
        Some(&after),
    )?;

    // 7. Commit
    conn.execute("COMMIT", [])?;

    Ok(after)
}
```

### Field Diff Calculation

```rust
fn calculate_field_changes(
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> serde_json::Value {
    match (before, after) {
        (None, Some(new)) => {
            // CREATE: all fields are additions
            let obj = new.as_object().unwrap();
            obj.iter()
                .map(|(k, v)| (k.clone(), json!({"old": null, "new": v})))
                .collect()
        }
        (Some(old), None) => {
            // DELETE: all fields are removals
            let obj = old.as_object().unwrap();
            obj.iter()
                .map(|(k, v)| (k.clone(), json!({"old": v, "new": null})))
                .collect()
        }
        (Some(old), Some(new)) => {
            // UPDATE: only changed fields
            compare_json_objects(old, new)
        }
        _ => json!({})
    }
}
```

## API Endpoints

### Admin Endpoints (require `EditCatalog` permission)

| Method | Path                                       | Description                      |
| ------ | ------------------------------------------ | -------------------------------- |
| POST   | `/v1/admin/changelog/batch`                | Create new batch                 |
| GET    | `/v1/admin/changelog/batches?is_open=true` | List batches (filter by is_open) |
| GET    | `/v1/admin/changelog/batch/{id}`           | Get batch details                |
| POST   | `/v1/admin/changelog/batch/{id}/close`     | Close batch                      |
| DELETE | `/v1/admin/changelog/batch/{id}`           | Delete open batch                |
| GET    | `/v1/admin/changelog/batch/{id}/changes`   | Get all changes in batch         |
| GET    | `/v1/admin/changelog/entity/{type}/{id}`   | Get entity change history        |

### User Endpoints (require `AccessCatalog` permission)

| Method | Path                              | Description                              |
| ------ | --------------------------------- | ---------------------------------------- |
| GET    | `/v1/content/whatsnew`            | List closed batches with content summary |
| GET    | `/v1/content/whatsnew/{batch_id}` | Get batch details with full content      |

### Request/Response Examples

**Create Batch:**

```http
POST /v1/admin/changelog/batch
{"name": "December 2024 Releases", "description": "New content for December"}

Response:
{"id": "batch_abc123", "name": "December 2024 Releases", "is_open": true, "created_at": 1733000000}
```

**What's New (User):**

```http
GET /v1/content/whatsnew?limit=10

Response:
{
  "batches": [{
    "id": "batch_abc123",
    "name": "December 2024 Releases",
    "closed_at": 1733500000,
    "summary": {
      "artists": {
        "added": [
          {"id": "R123", "name": "The Beatles"},
          {"id": "R456", "name": "Pink Floyd"}
        ],
        "updated_count": 2,
        "deleted": []
      },
      "albums": {
        "added": [
          {"id": "A789", "name": "Abbey Road"},
          {"id": "A012", "name": "Dark Side of the Moon"}
        ],
        "updated_count": 1,
        "deleted": [
          {"id": "A999", "name": "Removed Album"}
        ]
      },
      "tracks": {
        "added_count": 42,
        "updated_count": 5,
        "deleted_count": 0
      }
    }
  }]
}
```

## Implementation Steps

### Phase 1: Schema and Core Types

1. Add schema v2 migration in `catalog_store/schema.rs`
2. Create `catalog_store/changelog.rs` module with models and enums
3. Create `ChangeLogStore` struct with batch CRUD operations
4. Add unit tests for batch operations

### Phase 2: Change Recording

1. Implement `record_change_internal` method
2. Implement field diff calculation (`calculate_field_changes`)
3. Implement `generate_display_summary` helper
4. Add unit tests for diff calculation

### Phase 3: CatalogStore Integration

1. Add `ChangeLogStore` to `SqliteCatalogStore`
2. Add `get_active_batch_internal` helper method
3. Add `CatalogError::NoBatchActive` error variant
4. Modify `create_artist`, `update_artist`, `delete_artist` to record changes
5. Modify `create_album`, `update_album`, `delete_album` to record changes
6. Modify `create_track`, `update_track`, `delete_track` to record changes
7. Modify `create_image`, `update_image`, `delete_image` to record changes
8. Add integration tests

### Phase 4: Admin API

1. Add batch management routes to `server.rs`
2. Add change query routes
3. Add API tests

### Phase 5: User API

1. Add `/v1/content/whatsnew` endpoint
2. Add resolved content fetching (include artist/album details)
3. Add API tests

### Phase 6: Frontend (Future)

1. Add "What's New" view component
2. Add to Vue router
3. Add to remote store

## Critical Files

| File                         | Changes                                 |
| ---------------------------- | --------------------------------------- |
| `catalog_store/schema.rs`    | Add v2 schema with changelog tables     |
| `catalog_store/changelog.rs` | NEW - ChangeLogStore and models         |
| `catalog_store/store.rs`     | Add ChangeLogStore, modify CRUD methods |
| `catalog_store/mod.rs`       | Export changelog module                 |
| `server/server.rs`           | Add admin and user endpoints            |
| `server/mod.rs`              | Export new handlers                     |

## Error Handling

New error variant for catalog operations:

```rust
pub enum CatalogError {
    // ... existing variants
    NoBatchActive,
    BatchNotFound(String),
    BatchAlreadyClosed(String),
    BatchAlreadyActive(String),
    BatchNotEmpty(String),  // Cannot delete batch with changes
}
```

HTTP status codes:
- `NoBatchActive`: 409 Conflict - must create a batch before catalog modifications
- `BatchNotFound`: 404 Not Found
- `BatchAlreadyClosed`: 409 Conflict - cannot modify or re-close a closed batch
- `BatchAlreadyActive`: 409 Conflict - cannot create a new batch while one is open
- `BatchNotEmpty`: 400 Bad Request - cannot delete batch that has recorded changes

## Stale Batch Alerting

When a batch remains open with no activity for 1 hour, trigger a Telegram alert via the existing alerts infrastructure.

**Implementation:**
- Background task checks for open batches where `now - last_activity_at > 1 hour`
- `last_activity_at` is set to `created_at` when batch is created
- `last_activity_at` is updated whenever a change is recorded to the batch
- Alert includes batch name, creation time, and time since last activity
- Alert is sent once per stale batch (track sent alerts to avoid spam)
