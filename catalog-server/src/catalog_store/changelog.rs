//! Catalog changelog models and types.
//!
//! This module defines the types for tracking changes to catalog entities
//! (artists, albums, tracks, images) through batched changelog entries.

use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// =============================================================================
// Enumerations
// =============================================================================

/// Operation type for a changelog entry
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
}

impl ChangeOperation {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "create" => ChangeOperation::Create,
            "update" => ChangeOperation::Update,
            "delete" => ChangeOperation::Delete,
            _ => ChangeOperation::Update, // Default fallback
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            ChangeOperation::Create => "create",
            ChangeOperation::Update => "update",
            ChangeOperation::Delete => "delete",
        }
    }
}

/// Entity type for a changelog entry
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ChangeEntityType {
    Artist,
    Album,
    Track,
    Image,
}

impl ChangeEntityType {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "artist" => ChangeEntityType::Artist,
            "album" => ChangeEntityType::Album,
            "track" => ChangeEntityType::Track,
            "image" => ChangeEntityType::Image,
            _ => ChangeEntityType::Artist, // Default fallback
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            ChangeEntityType::Artist => "artist",
            ChangeEntityType::Album => "album",
            ChangeEntityType::Track => "track",
            ChangeEntityType::Image => "image",
        }
    }

    /// Get the plural form for display purposes
    pub fn plural(&self) -> &'static str {
        match self {
            ChangeEntityType::Artist => "artists",
            ChangeEntityType::Album => "albums",
            ChangeEntityType::Track => "tracks",
            ChangeEntityType::Image => "images",
        }
    }
}

// =============================================================================
// Structs
// =============================================================================

/// A batch of catalog changes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogBatch {
    /// Unique identifier (UUID)
    pub id: String,
    /// Human-readable name for the batch
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Whether the batch is still open for changes
    pub is_open: bool,
    /// Unix timestamp when the batch was created
    pub created_at: i64,
    /// Unix timestamp when the batch was closed (None if still open)
    pub closed_at: Option<i64>,
    /// Unix timestamp of last activity (change recorded)
    pub last_activity_at: i64,
}

/// A single change entry in the changelog
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeEntry {
    /// Auto-incrementing ID
    pub id: i64,
    /// ID of the batch this change belongs to
    pub batch_id: String,
    /// Type of entity that was changed
    pub entity_type: ChangeEntityType,
    /// ID of the entity that was changed
    pub entity_id: String,
    /// Type of operation performed
    pub operation: ChangeOperation,
    /// JSON object with field-level changes: {"field": {"old": X, "new": Y}}
    pub field_changes: serde_json::Value,
    /// Full JSON snapshot of the entity after the change (before for deletes)
    pub entity_snapshot: serde_json::Value,
    /// Human-readable summary of the change
    pub display_summary: Option<String>,
    /// Unix timestamp when the change was recorded
    pub created_at: i64,
}

/// Summary of changes in a batch for the "What's New" endpoint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BatchChangeSummary {
    pub artists: EntityChangeSummary,
    pub albums: EntityChangeSummary,
    pub tracks: TrackChangeSummary,
    pub images: EntityChangeSummary,
}

/// Summary of changes for a single entity type (artists, albums, images)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntityChangeSummary {
    /// List of added entities with id and name
    pub added: Vec<EntityRef>,
    /// Count of updated entities
    pub updated_count: usize,
    /// List of deleted entities with id and name
    pub deleted: Vec<EntityRef>,
}

/// Summary of track changes (counts only due to volume)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrackChangeSummary {
    pub added_count: usize,
    pub updated_count: usize,
    pub deleted_count: usize,
}

/// Reference to an entity with id and name
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: String,
    pub name: String,
}

/// Input for creating a new batch
#[derive(Clone, Debug, Deserialize)]
pub struct CreateBatchRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Response for the "What's New" endpoint
#[derive(Clone, Debug, Serialize)]
pub struct WhatsNewResponse {
    pub batches: Vec<WhatsNewBatch>,
}

/// A batch in the "What's New" response
#[derive(Clone, Debug, Serialize)]
pub struct WhatsNewBatch {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub closed_at: i64,
    pub summary: BatchChangeSummary,
}

// =============================================================================
// ChangeLogStore
// =============================================================================

/// Store for managing catalog changelog batches and entries.
///
/// Shares the same database connection as SqliteCatalogStore.
#[derive(Clone)]
pub struct ChangeLogStore {
    conn: Arc<Mutex<Connection>>,
}

impl ChangeLogStore {
    /// Create a new ChangeLogStore with a shared connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Get the current Unix timestamp.
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    // =========================================================================
    // Batch Management
    // =========================================================================

    /// Create a new batch. Fails if there's already an open batch.
    pub fn create_batch(&self, name: &str, description: Option<&str>) -> Result<CatalogBatch> {
        let conn = self.conn.lock().unwrap();

        // Check if there's already an active batch
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM catalog_batches WHERE is_open = 1",
            [],
            |row| row.get(0),
        )?;

        if active_count > 0 {
            bail!("Cannot create batch: another batch is already open");
        }

        let id = Uuid::new_v4().to_string();
        let now = Self::now();

        conn.execute(
            "INSERT INTO catalog_batches (id, name, description, is_open, created_at, last_activity_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            params![id, name, description, now],
        )?;

        Ok(CatalogBatch {
            id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            is_open: true,
            created_at: now,
            closed_at: None,
            last_activity_at: now,
        })
    }

    /// Get a batch by ID.
    pub fn get_batch(&self, id: &str) -> Result<Option<CatalogBatch>> {
        let conn = self.conn.lock().unwrap();
        self.get_batch_internal(&conn, id)
    }

    /// Internal method to get a batch using an existing connection lock.
    fn get_batch_internal(&self, conn: &Connection, id: &str) -> Result<Option<CatalogBatch>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
             FROM catalog_batches WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], |row| {
            Ok(CatalogBatch {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_open: row.get::<_, i64>(3)? == 1,
                created_at: row.get(4)?,
                closed_at: row.get(5)?,
                last_activity_at: row.get(6)?,
            })
        }) {
            Ok(batch) => Ok(Some(batch)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the currently active (open) batch, if any.
    pub fn get_active_batch(&self) -> Result<Option<CatalogBatch>> {
        let conn = self.conn.lock().unwrap();
        self.get_active_batch_internal(&conn)
    }

    /// Internal method to get active batch using an existing connection lock.
    pub fn get_active_batch_internal(&self, conn: &Connection) -> Result<Option<CatalogBatch>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
             FROM catalog_batches WHERE is_open = 1 LIMIT 1",
        )?;

        match stmt.query_row([], |row| {
            Ok(CatalogBatch {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_open: row.get::<_, i64>(3)? == 1,
                created_at: row.get(4)?,
                closed_at: row.get(5)?,
                last_activity_at: row.get(6)?,
            })
        }) {
            Ok(batch) => Ok(Some(batch)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Close a batch. Fails if batch doesn't exist or is already closed.
    pub fn close_batch(&self, id: &str) -> Result<CatalogBatch> {
        let conn = self.conn.lock().unwrap();

        let batch = self.get_batch_internal(&conn, id)?;
        let batch = match batch {
            Some(b) => b,
            None => bail!("Batch not found: {}", id),
        };

        if !batch.is_open {
            bail!("Batch is already closed: {}", id);
        }

        let now = Self::now();
        conn.execute(
            "UPDATE catalog_batches SET is_open = 0, closed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;

        Ok(CatalogBatch {
            is_open: false,
            closed_at: Some(now),
            ..batch
        })
    }

    /// List batches, optionally filtered by open/closed status.
    pub fn list_batches(&self, is_open: Option<bool>) -> Result<Vec<CatalogBatch>> {
        let conn = self.conn.lock().unwrap();

        let sql = match is_open {
            Some(true) => "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                          FROM catalog_batches WHERE is_open = 1 ORDER BY created_at DESC",
            Some(false) => "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                           FROM catalog_batches WHERE is_open = 0 ORDER BY closed_at DESC",
            None => "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                    FROM catalog_batches ORDER BY created_at DESC",
        };

        let mut stmt = conn.prepare(sql)?;
        let batches = stmt
            .query_map([], |row| {
                Ok(CatalogBatch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    is_open: row.get::<_, i64>(3)? == 1,
                    created_at: row.get(4)?,
                    closed_at: row.get(5)?,
                    last_activity_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(batches)
    }

    /// Delete a batch. Only allowed if batch is open and has no changes.
    pub fn delete_batch(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let batch = self.get_batch_internal(&conn, id)?;
        let batch = match batch {
            Some(b) => b,
            None => bail!("Batch not found: {}", id),
        };

        if !batch.is_open {
            bail!("Cannot delete closed batch: {}", id);
        }

        // Check if batch has any changes
        let change_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM catalog_change_log WHERE batch_id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        if change_count > 0 {
            bail!(
                "Cannot delete batch with {} recorded changes: {}",
                change_count,
                id
            );
        }

        conn.execute("DELETE FROM catalog_batches WHERE id = ?1", params![id])?;

        Ok(())
    }

    /// Get the count of changes in a batch.
    pub fn get_batch_change_count(&self, batch_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM catalog_change_log WHERE batch_id = ?1",
            params![batch_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Update the last_activity_at timestamp for a batch.
    /// Called internally when recording changes.
    pub fn update_batch_activity_internal(&self, conn: &Connection, batch_id: &str) -> Result<()> {
        let now = Self::now();
        conn.execute(
            "UPDATE catalog_batches SET last_activity_at = ?1 WHERE id = ?2",
            params![now, batch_id],
        )?;
        Ok(())
    }

    // =========================================================================
    // Change Queries
    // =========================================================================

    /// Get all changes in a batch.
    pub fn get_batch_changes(&self, batch_id: &str) -> Result<Vec<ChangeEntry>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, batch_id, entity_type, entity_id, operation, field_changes,
                    entity_snapshot, display_summary, created_at
             FROM catalog_change_log WHERE batch_id = ?1 ORDER BY created_at ASC",
        )?;

        let entries = stmt
            .query_map(params![batch_id], |row| {
                let field_changes_str: String = row.get(5)?;
                let entity_snapshot_str: String = row.get(6)?;

                Ok(ChangeEntry {
                    id: row.get(0)?,
                    batch_id: row.get(1)?,
                    entity_type: ChangeEntityType::from_db_str(&row.get::<_, String>(2)?),
                    entity_id: row.get(3)?,
                    operation: ChangeOperation::from_db_str(&row.get::<_, String>(4)?),
                    field_changes: serde_json::from_str(&field_changes_str).unwrap_or_default(),
                    entity_snapshot: serde_json::from_str(&entity_snapshot_str).unwrap_or_default(),
                    display_summary: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Get change history for a specific entity.
    pub fn get_entity_history(
        &self,
        entity_type: ChangeEntityType,
        entity_id: &str,
    ) -> Result<Vec<ChangeEntry>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, batch_id, entity_type, entity_id, operation, field_changes,
                    entity_snapshot, display_summary, created_at
             FROM catalog_change_log
             WHERE entity_type = ?1 AND entity_id = ?2
             ORDER BY created_at DESC",
        )?;

        let entries = stmt
            .query_map(params![entity_type.to_db_str(), entity_id], |row| {
                let field_changes_str: String = row.get(5)?;
                let entity_snapshot_str: String = row.get(6)?;

                Ok(ChangeEntry {
                    id: row.get(0)?,
                    batch_id: row.get(1)?,
                    entity_type: ChangeEntityType::from_db_str(&row.get::<_, String>(2)?),
                    entity_id: row.get(3)?,
                    operation: ChangeOperation::from_db_str(&row.get::<_, String>(4)?),
                    field_changes: serde_json::from_str(&field_changes_str).unwrap_or_default(),
                    entity_snapshot: serde_json::from_str(&entity_snapshot_str).unwrap_or_default(),
                    display_summary: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::schema::CATALOG_VERSIONED_SCHEMAS;

    /// Helper to create an in-memory database with the changelog schema
    fn create_test_store() -> ChangeLogStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        // Use v2 schema which includes changelog tables
        CATALOG_VERSIONED_SCHEMAS[2].create(&conn).unwrap();
        ChangeLogStore::new(Arc::new(Mutex::new(conn)))
    }

    // =========================================================================
    // Enum conversion tests
    // =========================================================================

    #[test]
    fn test_change_operation_db_conversion() {
        assert_eq!(ChangeOperation::from_db_str("create"), ChangeOperation::Create);
        assert_eq!(ChangeOperation::from_db_str("update"), ChangeOperation::Update);
        assert_eq!(ChangeOperation::from_db_str("delete"), ChangeOperation::Delete);

        assert_eq!(ChangeOperation::Create.to_db_str(), "create");
        assert_eq!(ChangeOperation::Update.to_db_str(), "update");
        assert_eq!(ChangeOperation::Delete.to_db_str(), "delete");
    }

    #[test]
    fn test_change_entity_type_db_conversion() {
        assert_eq!(ChangeEntityType::from_db_str("artist"), ChangeEntityType::Artist);
        assert_eq!(ChangeEntityType::from_db_str("album"), ChangeEntityType::Album);
        assert_eq!(ChangeEntityType::from_db_str("track"), ChangeEntityType::Track);
        assert_eq!(ChangeEntityType::from_db_str("image"), ChangeEntityType::Image);

        assert_eq!(ChangeEntityType::Artist.to_db_str(), "artist");
        assert_eq!(ChangeEntityType::Album.to_db_str(), "album");
        assert_eq!(ChangeEntityType::Track.to_db_str(), "track");
        assert_eq!(ChangeEntityType::Image.to_db_str(), "image");
    }

    #[test]
    fn test_change_entity_type_plural() {
        assert_eq!(ChangeEntityType::Artist.plural(), "artists");
        assert_eq!(ChangeEntityType::Album.plural(), "albums");
        assert_eq!(ChangeEntityType::Track.plural(), "tracks");
        assert_eq!(ChangeEntityType::Image.plural(), "images");
    }

    // =========================================================================
    // Batch CRUD tests
    // =========================================================================

    #[test]
    fn test_create_batch_success() {
        let store = create_test_store();

        let batch = store.create_batch("Test Batch", Some("A test description")).unwrap();

        assert!(!batch.id.is_empty());
        assert_eq!(batch.name, "Test Batch");
        assert_eq!(batch.description, Some("A test description".to_string()));
        assert!(batch.is_open);
        assert!(batch.closed_at.is_none());
        assert!(batch.created_at > 0);
        assert_eq!(batch.created_at, batch.last_activity_at);
    }

    #[test]
    fn test_create_batch_without_description() {
        let store = create_test_store();

        let batch = store.create_batch("Minimal Batch", None).unwrap();

        assert_eq!(batch.name, "Minimal Batch");
        assert!(batch.description.is_none());
    }

    #[test]
    fn test_create_batch_fails_when_one_already_open() {
        let store = create_test_store();

        // Create first batch
        store.create_batch("First Batch", None).unwrap();

        // Try to create second batch - should fail
        let result = store.create_batch("Second Batch", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already open"));
    }

    #[test]
    fn test_get_batch_found() {
        let store = create_test_store();

        let created = store.create_batch("Test Batch", Some("desc")).unwrap();
        let fetched = store.get_batch(&created.id).unwrap();

        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test Batch");
        assert_eq!(fetched.description, Some("desc".to_string()));
    }

    #[test]
    fn test_get_batch_not_found() {
        let store = create_test_store();

        let result = store.get_batch("nonexistent-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_active_batch_when_exists() {
        let store = create_test_store();

        let created = store.create_batch("Active Batch", None).unwrap();
        let active = store.get_active_batch().unwrap();

        assert!(active.is_some());
        assert_eq!(active.unwrap().id, created.id);
    }

    #[test]
    fn test_get_active_batch_when_none() {
        let store = create_test_store();

        let active = store.get_active_batch().unwrap();
        assert!(active.is_none());
    }

    #[test]
    fn test_get_active_batch_after_close() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();
        store.close_batch(&batch.id).unwrap();

        let active = store.get_active_batch().unwrap();
        assert!(active.is_none());
    }

    #[test]
    fn test_close_batch_success() {
        let store = create_test_store();

        let batch = store.create_batch("Batch to Close", None).unwrap();
        let closed = store.close_batch(&batch.id).unwrap();

        assert!(!closed.is_open);
        assert!(closed.closed_at.is_some());
        assert!(closed.closed_at.unwrap() >= closed.created_at);
    }

    #[test]
    fn test_close_batch_not_found() {
        let store = create_test_store();

        let result = store.close_batch("nonexistent-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_close_batch_already_closed() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();
        store.close_batch(&batch.id).unwrap();

        let result = store.close_batch(&batch.id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already closed"));
    }

    #[test]
    fn test_list_batches_empty() {
        let store = create_test_store();

        let batches = store.list_batches(None).unwrap();
        assert!(batches.is_empty());
    }

    #[test]
    fn test_list_batches_all() {
        let store = create_test_store();

        // Create and close first batch
        let batch1 = store.create_batch("Batch 1", None).unwrap();
        store.close_batch(&batch1.id).unwrap();

        // Create second batch (open)
        let _batch2 = store.create_batch("Batch 2", None).unwrap();

        let batches = store.list_batches(None).unwrap();
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_list_batches_filter_open() {
        let store = create_test_store();

        // Create and close first batch
        let batch1 = store.create_batch("Closed Batch", None).unwrap();
        store.close_batch(&batch1.id).unwrap();

        // Create second batch (open)
        store.create_batch("Open Batch", None).unwrap();

        let open_batches = store.list_batches(Some(true)).unwrap();
        assert_eq!(open_batches.len(), 1);
        assert_eq!(open_batches[0].name, "Open Batch");
    }

    #[test]
    fn test_list_batches_filter_closed() {
        let store = create_test_store();

        // Create and close first batch
        let batch1 = store.create_batch("Closed Batch", None).unwrap();
        store.close_batch(&batch1.id).unwrap();

        // Create second batch (open)
        store.create_batch("Open Batch", None).unwrap();

        let closed_batches = store.list_batches(Some(false)).unwrap();
        assert_eq!(closed_batches.len(), 1);
        assert_eq!(closed_batches[0].name, "Closed Batch");
    }

    #[test]
    fn test_delete_batch_success() {
        let store = create_test_store();

        let batch = store.create_batch("To Delete", None).unwrap();
        store.delete_batch(&batch.id).unwrap();

        // Verify it's gone
        let fetched = store.get_batch(&batch.id).unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn test_delete_batch_not_found() {
        let store = create_test_store();

        let result = store.delete_batch("nonexistent-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_delete_batch_fails_when_closed() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();
        store.close_batch(&batch.id).unwrap();

        let result = store.delete_batch(&batch.id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("closed"));
    }

    #[test]
    fn test_delete_batch_fails_when_has_changes() {
        let store = create_test_store();

        let batch = store.create_batch("Batch with Changes", None).unwrap();

        // Manually insert a change entry
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'R1', 'create', '{}', '{\"id\":\"R1\"}', ?2)",
                params![batch.id, ChangeLogStore::now()],
            ).unwrap();
        }

        let result = store.delete_batch(&batch.id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("recorded changes"));
    }

    #[test]
    fn test_get_batch_change_count() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();

        // Initially zero
        assert_eq!(store.get_batch_change_count(&batch.id).unwrap(), 0);

        // Add some changes manually
        {
            let conn = store.conn.lock().unwrap();
            let now = ChangeLogStore::now();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'R1', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'album', 'A1', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
        }

        assert_eq!(store.get_batch_change_count(&batch.id).unwrap(), 2);
    }

    #[test]
    fn test_can_create_batch_after_previous_closed() {
        let store = create_test_store();

        // Create and close first batch
        let batch1 = store.create_batch("First", None).unwrap();
        store.close_batch(&batch1.id).unwrap();

        // Should be able to create another
        let batch2 = store.create_batch("Second", None).unwrap();
        assert!(batch2.is_open);
        assert_ne!(batch1.id, batch2.id);
    }

    #[test]
    fn test_batch_id_is_uuid_format() {
        let store = create_test_store();

        let batch = store.create_batch("Test", None).unwrap();

        // UUID v4 format: 8-4-4-4-12 hex chars
        let parts: Vec<&str> = batch.id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    // =========================================================================
    // Change query tests
    // =========================================================================

    #[test]
    fn test_get_batch_changes_empty() {
        let store = create_test_store();

        let batch = store.create_batch("Empty Batch", None).unwrap();
        let changes = store.get_batch_changes(&batch.id).unwrap();

        assert!(changes.is_empty());
    }

    #[test]
    fn test_get_batch_changes_with_entries() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();

        // Add changes manually
        {
            let conn = store.conn.lock().unwrap();
            let now = ChangeLogStore::now();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, display_summary, created_at)
                 VALUES (?1, 'artist', 'R1', 'create', '{\"name\":{\"old\":null,\"new\":\"Test Artist\"}}', '{\"id\":\"R1\",\"name\":\"Test Artist\"}', 'Created artist Test Artist', ?2)",
                params![batch.id, now],
            ).unwrap();
        }

        let changes = store.get_batch_changes(&batch.id).unwrap();
        assert_eq!(changes.len(), 1);

        let change = &changes[0];
        assert_eq!(change.batch_id, batch.id);
        assert_eq!(change.entity_type, ChangeEntityType::Artist);
        assert_eq!(change.entity_id, "R1");
        assert_eq!(change.operation, ChangeOperation::Create);
        assert_eq!(change.display_summary, Some("Created artist Test Artist".to_string()));
    }

    #[test]
    fn test_get_entity_history() {
        let store = create_test_store();

        // Create two batches with changes to the same entity
        let batch1 = store.create_batch("Batch 1", None).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'R1', 'create', '{}', '{}', 1000)",
                params![batch1.id],
            ).unwrap();
        }
        store.close_batch(&batch1.id).unwrap();

        let batch2 = store.create_batch("Batch 2", None).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'R1', 'update', '{}', '{}', 2000)",
                params![batch2.id],
            ).unwrap();
        }

        let history = store.get_entity_history(ChangeEntityType::Artist, "R1").unwrap();
        assert_eq!(history.len(), 2);
        // Should be ordered by created_at DESC (most recent first)
        assert_eq!(history[0].operation, ChangeOperation::Update);
        assert_eq!(history[1].operation, ChangeOperation::Create);
    }

    #[test]
    fn test_get_entity_history_filters_by_type() {
        let store = create_test_store();

        let batch = store.create_batch("Batch", None).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            let now = ChangeLogStore::now();
            // Add artist change
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'R1', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
            // Add album change with same ID
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'album', 'R1', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
        }

        let artist_history = store.get_entity_history(ChangeEntityType::Artist, "R1").unwrap();
        assert_eq!(artist_history.len(), 1);
        assert_eq!(artist_history[0].entity_type, ChangeEntityType::Artist);

        let album_history = store.get_entity_history(ChangeEntityType::Album, "R1").unwrap();
        assert_eq!(album_history.len(), 1);
        assert_eq!(album_history[0].entity_type, ChangeEntityType::Album);
    }
}
