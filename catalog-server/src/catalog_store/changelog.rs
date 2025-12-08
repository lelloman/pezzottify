//! Catalog changelog models and types.
//!
//! This module defines the types for tracking changes to catalog entities
//! (artists, albums, tracks, images) through batched changelog entries.
#![allow(dead_code)]

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
// Field Diff Calculation
// =============================================================================

/// Calculate the diff between two JSON values representing entity states.
///
/// For CREATE operations, pass `None` as `old`.
/// For DELETE operations, pass `None` as `new`.
/// For UPDATE operations, pass both values.
///
/// Returns a JSON object with changed fields: `{"field": {"old": X, "new": Y}}`
pub fn calculate_field_diff(
    old: Option<&serde_json::Value>,
    new: Option<&serde_json::Value>,
) -> serde_json::Value {
    match (old, new) {
        // CREATE: all fields are new
        (None, Some(new_val)) => {
            if let Some(obj) = new_val.as_object() {
                let mut diff = serde_json::Map::new();
                for (key, value) in obj {
                    diff.insert(key.clone(), serde_json::json!({"old": null, "new": value}));
                }
                serde_json::Value::Object(diff)
            } else {
                serde_json::json!({"value": {"old": null, "new": new_val}})
            }
        }
        // DELETE: all fields are removed
        (Some(old_val), None) => {
            if let Some(obj) = old_val.as_object() {
                let mut diff = serde_json::Map::new();
                for (key, value) in obj {
                    diff.insert(key.clone(), serde_json::json!({"old": value, "new": null}));
                }
                serde_json::Value::Object(diff)
            } else {
                serde_json::json!({"value": {"old": old_val, "new": null}})
            }
        }
        // UPDATE: compare fields
        (Some(old_val), Some(new_val)) => calculate_object_diff(old_val, new_val),
        // Both None - shouldn't happen, return empty diff
        (None, None) => serde_json::json!({}),
    }
}

/// Generate a human-readable display summary for a change.
///
/// # Arguments
/// * `entity_type` - Type of entity (Artist, Album, Track, Image)
/// * `operation` - Type of operation (Create, Update, Delete)
/// * `entity_name` - Name of the entity (extracted from snapshot)
/// * `field_changes` - The diff object (for updates, to summarize changed fields)
///
/// # Returns
/// A human-readable summary string like "Created artist The Beatles" or "Updated album Abbey Road"
pub fn generate_display_summary(
    entity_type: &ChangeEntityType,
    operation: &ChangeOperation,
    entity_name: Option<&str>,
) -> String {
    let type_str = entity_type.to_db_str();
    let name = entity_name.unwrap_or("(unknown)");

    match operation {
        ChangeOperation::Create => format!("Created {} '{}'", type_str, name),
        ChangeOperation::Update => format!("Updated {} '{}'", type_str, name),
        ChangeOperation::Delete => format!("Deleted {} '{}'", type_str, name),
    }
}

/// Extract the name field from an entity snapshot.
///
/// Looks for common name fields: "name", "title" (for tracks/albums).
pub fn extract_entity_name(snapshot: &serde_json::Value) -> Option<String> {
    if let Some(obj) = snapshot.as_object() {
        // Try "name" first (artists, albums)
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
        // Try "title" (tracks might use this)
        if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
            return Some(title.to_string());
        }
    }
    None
}

/// Calculate diff between two JSON objects, returning only changed fields.
fn calculate_object_diff(old: &serde_json::Value, new: &serde_json::Value) -> serde_json::Value {
    let mut diff = serde_json::Map::new();

    let old_obj = old.as_object();
    let new_obj = new.as_object();

    match (old_obj, new_obj) {
        (Some(old_map), Some(new_map)) => {
            // Check all keys in old
            for (key, old_value) in old_map {
                match new_map.get(key) {
                    Some(new_value) => {
                        if old_value != new_value {
                            diff.insert(
                                key.clone(),
                                serde_json::json!({"old": old_value, "new": new_value}),
                            );
                        }
                    }
                    None => {
                        // Key was removed
                        diff.insert(
                            key.clone(),
                            serde_json::json!({"old": old_value, "new": null}),
                        );
                    }
                }
            }
            // Check for new keys
            for (key, new_value) in new_map {
                if !old_map.contains_key(key) {
                    diff.insert(
                        key.clone(),
                        serde_json::json!({"old": null, "new": new_value}),
                    );
                }
            }
        }
        _ => {
            // Not both objects, treat as simple value change
            if old != new {
                diff.insert(
                    "value".to_string(),
                    serde_json::json!({"old": old, "new": new}),
                );
            }
        }
    }

    serde_json::Value::Object(diff)
}

// =============================================================================
// ChangeLogStore
// =============================================================================

/// Default inactivity threshold (in seconds) after which a batch is considered stale.
/// Stale batches are automatically closed when a new change is recorded.
pub const DEFAULT_BATCH_INACTIVITY_THRESHOLD_SECS: i64 = 3600; // 1 hour

/// Store for managing catalog changelog batches and entries.
///
/// Shares the same database connection as SqliteCatalogStore.
#[derive(Clone)]
pub struct ChangeLogStore {
    conn: Arc<Mutex<Connection>>,
    /// Inactivity threshold in seconds for auto-closing batches.
    batch_inactivity_threshold_secs: i64,
}

impl ChangeLogStore {
    /// Create a new ChangeLogStore with a shared connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            batch_inactivity_threshold_secs: DEFAULT_BATCH_INACTIVITY_THRESHOLD_SECS,
        }
    }

    /// Create a new ChangeLogStore with a custom inactivity threshold.
    pub fn with_inactivity_threshold(conn: Arc<Mutex<Connection>>, threshold_secs: i64) -> Self {
        Self {
            conn,
            batch_inactivity_threshold_secs: threshold_secs,
        }
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

    /// Internal method to close a batch using an existing connection lock.
    fn close_batch_internal(&self, conn: &Connection, id: &str) -> Result<CatalogBatch> {
        let batch = self.get_batch_internal(conn, id)?;
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

    /// Generate a date-based name for auto-created batches.
    fn generate_auto_batch_name() -> String {
        use chrono::{DateTime, Utc};
        let now: DateTime<Utc> = Utc::now();
        now.format("%Y-%m-%d").to_string()
    }

    /// Internal method to create a batch using an existing connection lock.
    /// Does NOT check for existing open batches - caller is responsible.
    fn create_batch_internal(
        &self,
        conn: &Connection,
        name: &str,
        description: Option<&str>,
    ) -> Result<CatalogBatch> {
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

    /// Ensure an active batch exists, creating one if needed.
    ///
    /// This method implements automatic batch management:
    /// - If an open batch exists and is not stale, returns it
    /// - If an open batch exists but is stale (inactive > threshold), closes it and creates a new one
    /// - If no open batch exists, creates a new one
    ///
    /// Auto-created batches are named with the current date (YYYY-MM-DD).
    pub fn ensure_active_batch_internal(&self, conn: &Connection) -> Result<CatalogBatch> {
        // Check for existing open batch
        if let Some(batch) = self.get_active_batch_internal(conn)? {
            let now = Self::now();
            let inactive_seconds = now - batch.last_activity_at;

            if inactive_seconds > self.batch_inactivity_threshold_secs {
                // Batch is stale, close it and create a new one
                tracing::info!(
                    "Closing stale batch '{}' (inactive for {} seconds)",
                    batch.name,
                    inactive_seconds
                );
                self.close_batch_internal(conn, &batch.id)?;
            } else {
                // Batch is still active
                return Ok(batch);
            }
        }

        // Create a new auto-batch
        let name = Self::generate_auto_batch_name();
        tracing::info!("Creating new auto-batch '{}'", name);
        self.create_batch_internal(conn, &name, None)
    }

    /// Close any stale open batches.
    ///
    /// This is intended to be called periodically by a background task.
    /// Returns the number of batches that were closed.
    pub fn close_stale_batches(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let cutoff = now - self.batch_inactivity_threshold_secs;

        // Find and close stale open batches
        let mut stmt = conn.prepare(
            "SELECT id, name FROM catalog_batches WHERE is_open = 1 AND last_activity_at < ?1",
        )?;

        let stale_batches: Vec<(String, String)> = stmt
            .query_map(params![cutoff], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = stale_batches.len();
        for (id, name) in stale_batches {
            tracing::info!("Background task closing stale batch '{}'", name);
            conn.execute(
                "UPDATE catalog_batches SET is_open = 0, closed_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
        }

        Ok(count)
    }

    /// List batches, optionally filtered by open/closed status.
    pub fn list_batches(&self, is_open: Option<bool>) -> Result<Vec<CatalogBatch>> {
        let conn = self.conn.lock().unwrap();

        let sql = match is_open {
            Some(true) => {
                "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                          FROM catalog_batches WHERE is_open = 1 ORDER BY created_at DESC"
            }
            Some(false) => {
                "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                           FROM catalog_batches WHERE is_open = 0 ORDER BY closed_at DESC"
            }
            None => {
                "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
                    FROM catalog_batches ORDER BY created_at DESC"
            }
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
    // Change Recording
    // =========================================================================

    /// Record a change to an entity within the active batch.
    ///
    /// This is the internal method called during catalog write operations.
    /// It requires an existing connection lock to participate in the same transaction.
    ///
    /// If no active batch exists, one is automatically created with a date-based name.
    /// If the active batch is stale (inactive for longer than the threshold), it is
    /// closed and a new batch is created.
    ///
    /// # Arguments
    /// * `conn` - Locked database connection (for transaction safety)
    /// * `entity_type` - Type of entity being changed
    /// * `entity_id` - ID of the entity being changed
    /// * `operation` - Type of operation (Create, Update, Delete)
    /// * `field_changes` - JSON object with field-level changes
    /// * `entity_snapshot` - Full JSON snapshot of the entity
    /// * `display_summary` - Human-readable summary of the change
    ///
    /// # Returns
    /// The ID of the inserted change entry.
    #[allow(clippy::too_many_arguments)]
    pub fn record_change_internal(
        &self,
        conn: &Connection,
        entity_type: ChangeEntityType,
        entity_id: &str,
        operation: ChangeOperation,
        field_changes: &serde_json::Value,
        entity_snapshot: &serde_json::Value,
        display_summary: Option<&str>,
    ) -> Result<i64> {
        // Ensure we have an active batch (auto-create if needed)
        let batch = self.ensure_active_batch_internal(conn)?;

        let now = Self::now();
        let field_changes_str = serde_json::to_string(field_changes)?;
        let entity_snapshot_str = serde_json::to_string(entity_snapshot)?;

        conn.execute(
            "INSERT INTO catalog_change_log
             (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, display_summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                batch.id,
                entity_type.to_db_str(),
                entity_id,
                operation.to_db_str(),
                field_changes_str,
                entity_snapshot_str,
                display_summary,
                now,
            ],
        )?;

        let change_id = conn.last_insert_rowid();

        // Update batch activity timestamp
        self.update_batch_activity_internal(conn, &batch.id)?;

        Ok(change_id)
    }

    /// Record a change using the store's own connection (for standalone use).
    ///
    /// This method acquires its own lock and is suitable for use outside of
    /// existing transactions.
    pub fn record_change(
        &self,
        entity_type: ChangeEntityType,
        entity_id: &str,
        operation: ChangeOperation,
        field_changes: &serde_json::Value,
        entity_snapshot: &serde_json::Value,
        display_summary: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        self.record_change_internal(
            &conn,
            entity_type,
            entity_id,
            operation,
            field_changes,
            entity_snapshot,
            display_summary,
        )
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

    // =========================================================================
    // What's New Endpoint Support
    // =========================================================================

    /// Get a summary of changes in a batch.
    ///
    /// Aggregates all changes in the batch into a summary with:
    /// - Artists: added (id, name), updated count, deleted (id, name)
    /// - Albums: added (id, name), updated count, deleted (id, name)
    /// - Tracks: added count, updated count, deleted count
    /// - Images: added count, updated count, deleted count (not shown in What's New)
    pub fn get_batch_summary(&self, batch_id: &str) -> Result<BatchChangeSummary> {
        let changes = self.get_batch_changes(batch_id)?;
        Ok(Self::aggregate_changes_to_summary(&changes))
    }

    /// Aggregate a list of changes into a summary.
    fn aggregate_changes_to_summary(changes: &[ChangeEntry]) -> BatchChangeSummary {
        let mut summary = BatchChangeSummary::default();

        for change in changes {
            // Extract name from entity snapshot
            let name = change
                .entity_snapshot
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let entity_ref = EntityRef {
                id: change.entity_id.clone(),
                name,
            };

            match change.entity_type {
                ChangeEntityType::Artist => match change.operation {
                    ChangeOperation::Create => summary.artists.added.push(entity_ref),
                    ChangeOperation::Update => summary.artists.updated_count += 1,
                    ChangeOperation::Delete => summary.artists.deleted.push(entity_ref),
                },
                ChangeEntityType::Album => match change.operation {
                    ChangeOperation::Create => summary.albums.added.push(entity_ref),
                    ChangeOperation::Update => summary.albums.updated_count += 1,
                    ChangeOperation::Delete => summary.albums.deleted.push(entity_ref),
                },
                ChangeEntityType::Track => match change.operation {
                    ChangeOperation::Create => summary.tracks.added_count += 1,
                    ChangeOperation::Update => summary.tracks.updated_count += 1,
                    ChangeOperation::Delete => summary.tracks.deleted_count += 1,
                },
                ChangeEntityType::Image => match change.operation {
                    ChangeOperation::Create => summary.images.added.push(entity_ref),
                    ChangeOperation::Update => summary.images.updated_count += 1,
                    ChangeOperation::Delete => summary.images.deleted.push(entity_ref),
                },
            }
        }

        summary
    }

    /// Get closed batches with their summaries for the What's New endpoint.
    ///
    /// Returns batches in descending order by closed_at (most recent first).
    pub fn get_whats_new_batches(&self, limit: usize) -> Result<Vec<WhatsNewBatch>> {
        let conn = self.conn.lock().unwrap();

        // Get closed batches
        let mut stmt = conn.prepare(
            "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
             FROM catalog_batches
             WHERE is_open = 0
             ORDER BY closed_at DESC
             LIMIT ?1",
        )?;

        let batches: Vec<CatalogBatch> = stmt
            .query_map(params![limit as i64], |row| {
                Ok(CatalogBatch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    is_open: row.get::<_, i32>(3)? != 0,
                    created_at: row.get(4)?,
                    closed_at: row.get(5)?,
                    last_activity_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Get summaries for each batch
        let mut result = Vec::with_capacity(batches.len());
        for batch in batches {
            let changes = self.get_batch_changes_internal(&conn, &batch.id)?;
            let summary = Self::aggregate_changes_to_summary(&changes);
            result.push(WhatsNewBatch {
                id: batch.id,
                name: batch.name,
                description: batch.description,
                closed_at: batch.closed_at.unwrap_or(0),
                summary,
            });
        }

        Ok(result)
    }

    /// Internal method to get batch changes with an existing connection.
    fn get_batch_changes_internal(
        &self,
        conn: &Connection,
        batch_id: &str,
    ) -> Result<Vec<ChangeEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, batch_id, entity_type, entity_id, operation, field_changes,
                    entity_snapshot, display_summary, created_at
             FROM catalog_change_log
             WHERE batch_id = ?1
             ORDER BY created_at ASC",
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

    /// Get batches that have been open longer than the specified threshold.
    ///
    /// A batch is considered "stale" if:
    /// - It is open (is_open = true)
    /// - It was created more than `stale_threshold_hours` ago
    ///
    /// Returns the stale batches ordered by created_at ascending (oldest first).
    pub fn get_stale_batches(&self, stale_threshold_hours: u64) -> Result<Vec<CatalogBatch>> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let cutoff = now - (stale_threshold_hours * 60 * 60) as i64;

        let mut stmt = conn.prepare(
            "SELECT id, name, description, is_open, created_at, closed_at, last_activity_at
             FROM catalog_batches
             WHERE is_open = 1 AND created_at < ?1
             ORDER BY created_at ASC",
        )?;

        let batches = stmt
            .query_map(params![cutoff], |row| {
                Ok(CatalogBatch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    is_open: row.get::<_, i32>(3)? != 0,
                    created_at: row.get(4)?,
                    closed_at: row.get(5)?,
                    last_activity_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(batches)
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
        assert_eq!(
            ChangeOperation::from_db_str("create"),
            ChangeOperation::Create
        );
        assert_eq!(
            ChangeOperation::from_db_str("update"),
            ChangeOperation::Update
        );
        assert_eq!(
            ChangeOperation::from_db_str("delete"),
            ChangeOperation::Delete
        );

        assert_eq!(ChangeOperation::Create.to_db_str(), "create");
        assert_eq!(ChangeOperation::Update.to_db_str(), "update");
        assert_eq!(ChangeOperation::Delete.to_db_str(), "delete");
    }

    #[test]
    fn test_change_entity_type_db_conversion() {
        assert_eq!(
            ChangeEntityType::from_db_str("artist"),
            ChangeEntityType::Artist
        );
        assert_eq!(
            ChangeEntityType::from_db_str("album"),
            ChangeEntityType::Album
        );
        assert_eq!(
            ChangeEntityType::from_db_str("track"),
            ChangeEntityType::Track
        );
        assert_eq!(
            ChangeEntityType::from_db_str("image"),
            ChangeEntityType::Image
        );

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

        let batch = store
            .create_batch("Test Batch", Some("A test description"))
            .unwrap();

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
                 VALUES (?1, 'artist', 'test_artist_001', 'create', '{}', '{\"id\":\"test_artist_001\"}', ?2)",
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
                 VALUES (?1, 'artist', 'test_artist_001', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'album', 'test_album_001', 'create', '{}', '{}', ?2)",
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
                 VALUES (?1, 'artist', 'test_artist_001', 'create', '{\"name\":{\"old\":null,\"new\":\"Test Artist\"}}', '{\"id\":\"test_artist_001\",\"name\":\"Test Artist\"}', 'Created artist Test Artist', ?2)",
                params![batch.id, now],
            ).unwrap();
        }

        let changes = store.get_batch_changes(&batch.id).unwrap();
        assert_eq!(changes.len(), 1);

        let change = &changes[0];
        assert_eq!(change.batch_id, batch.id);
        assert_eq!(change.entity_type, ChangeEntityType::Artist);
        assert_eq!(change.entity_id, "test_artist_001");
        assert_eq!(change.operation, ChangeOperation::Create);
        assert_eq!(
            change.display_summary,
            Some("Created artist Test Artist".to_string())
        );
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
                 VALUES (?1, 'artist', 'test_artist_001', 'create', '{}', '{}', 1000)",
                params![batch1.id],
            ).unwrap();
        }
        store.close_batch(&batch1.id).unwrap();

        let batch2 = store.create_batch("Batch 2", None).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'artist', 'test_artist_001', 'update', '{}', '{}', 2000)",
                params![batch2.id],
            ).unwrap();
        }

        let history = store
            .get_entity_history(ChangeEntityType::Artist, "test_artist_001")
            .unwrap();
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
                 VALUES (?1, 'artist', 'test_artist_001', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
            // Add album change with same ID
            conn.execute(
                "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
                 VALUES (?1, 'album', 'test_artist_001', 'create', '{}', '{}', ?2)",
                params![batch.id, now],
            ).unwrap();
        }

        let artist_history = store
            .get_entity_history(ChangeEntityType::Artist, "test_artist_001")
            .unwrap();
        assert_eq!(artist_history.len(), 1);
        assert_eq!(artist_history[0].entity_type, ChangeEntityType::Artist);

        let album_history = store
            .get_entity_history(ChangeEntityType::Album, "test_artist_001")
            .unwrap();
        assert_eq!(album_history.len(), 1);
        assert_eq!(album_history[0].entity_type, ChangeEntityType::Album);
    }

    // =========================================================================
    // Field diff calculation tests
    // =========================================================================

    #[test]
    fn test_diff_create_operation() {
        let new_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });

        let diff = calculate_field_diff(None, Some(&new_entity));

        assert_eq!(diff["id"]["old"], serde_json::Value::Null);
        assert_eq!(diff["id"]["new"], "test_artist_001");
        assert_eq!(diff["name"]["old"], serde_json::Value::Null);
        assert_eq!(diff["name"]["new"], "The Beatles");
    }

    #[test]
    fn test_diff_delete_operation() {
        let old_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });

        let diff = calculate_field_diff(Some(&old_entity), None);

        assert_eq!(diff["id"]["old"], "test_artist_001");
        assert_eq!(diff["id"]["new"], serde_json::Value::Null);
        assert_eq!(diff["name"]["old"], "The Beatles");
        assert_eq!(diff["name"]["new"], serde_json::Value::Null);
    }

    #[test]
    fn test_diff_update_operation_changed_fields() {
        let old_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beetles"
        });
        let new_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });

        let diff = calculate_field_diff(Some(&old_entity), Some(&new_entity));

        // id unchanged, should not be in diff
        assert!(diff.get("id").is_none());
        // name changed
        assert_eq!(diff["name"]["old"], "The Beetles");
        assert_eq!(diff["name"]["new"], "The Beatles");
    }

    #[test]
    fn test_diff_update_no_changes() {
        let entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });

        let diff = calculate_field_diff(Some(&entity), Some(&entity));

        // No changes, diff should be empty
        assert!(diff.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_diff_field_added() {
        let old_entity = serde_json::json!({
            "id": "test_artist_001"
        });
        let new_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });

        let diff = calculate_field_diff(Some(&old_entity), Some(&new_entity));

        assert!(diff.get("id").is_none()); // unchanged
        assert_eq!(diff["name"]["old"], serde_json::Value::Null);
        assert_eq!(diff["name"]["new"], "The Beatles");
    }

    #[test]
    fn test_diff_field_removed() {
        let old_entity = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });
        let new_entity = serde_json::json!({
            "id": "test_artist_001"
        });

        let diff = calculate_field_diff(Some(&old_entity), Some(&new_entity));

        assert!(diff.get("id").is_none()); // unchanged
        assert_eq!(diff["name"]["old"], "The Beatles");
        assert_eq!(diff["name"]["new"], serde_json::Value::Null);
    }

    #[test]
    fn test_diff_both_none() {
        let diff = calculate_field_diff(None, None);
        assert!(diff.as_object().unwrap().is_empty());
    }

    // =========================================================================
    // Display summary tests
    // =========================================================================

    #[test]
    fn test_display_summary_create() {
        let summary = generate_display_summary(
            &ChangeEntityType::Artist,
            &ChangeOperation::Create,
            Some("The Beatles"),
        );
        assert_eq!(summary, "Created artist 'The Beatles'");
    }

    #[test]
    fn test_display_summary_update() {
        let summary = generate_display_summary(
            &ChangeEntityType::Album,
            &ChangeOperation::Update,
            Some("Abbey Road"),
        );
        assert_eq!(summary, "Updated album 'Abbey Road'");
    }

    #[test]
    fn test_display_summary_delete() {
        let summary = generate_display_summary(
            &ChangeEntityType::Track,
            &ChangeOperation::Delete,
            Some("Come Together"),
        );
        assert_eq!(summary, "Deleted track 'Come Together'");
    }

    #[test]
    fn test_display_summary_unknown_name() {
        let summary =
            generate_display_summary(&ChangeEntityType::Image, &ChangeOperation::Create, None);
        assert_eq!(summary, "Created image '(unknown)'");
    }

    #[test]
    fn test_extract_entity_name_from_name_field() {
        let snapshot = serde_json::json!({
            "id": "test_artist_001",
            "name": "The Beatles"
        });
        assert_eq!(
            extract_entity_name(&snapshot),
            Some("The Beatles".to_string())
        );
    }

    #[test]
    fn test_extract_entity_name_from_title_field() {
        let snapshot = serde_json::json!({
            "id": "test_track_001",
            "title": "Come Together"
        });
        assert_eq!(
            extract_entity_name(&snapshot),
            Some("Come Together".to_string())
        );
    }

    #[test]
    fn test_extract_entity_name_prefers_name() {
        let snapshot = serde_json::json!({
            "id": "test_album_001",
            "name": "Abbey Road",
            "title": "Some Title"
        });
        assert_eq!(
            extract_entity_name(&snapshot),
            Some("Abbey Road".to_string())
        );
    }

    #[test]
    fn test_extract_entity_name_missing() {
        let snapshot = serde_json::json!({
            "id": "I1",
            "uri": "/images/cover.jpg"
        });
        assert_eq!(extract_entity_name(&snapshot), None);
    }

    // =========================================================================
    // record_change tests
    // =========================================================================

    #[test]
    fn test_record_change_success() {
        let store = create_test_store();

        // Create a batch
        let batch = store.create_batch("Test Batch", None).unwrap();

        // Record a change
        let snapshot = serde_json::json!({"id": "test_artist_001", "name": "The Beatles"});
        let diff = calculate_field_diff(None, Some(&snapshot));

        let change_id = store
            .record_change(
                ChangeEntityType::Artist,
                "test_artist_001",
                ChangeOperation::Create,
                &diff,
                &snapshot,
                Some("Created artist 'The Beatles'"),
            )
            .unwrap();

        assert!(change_id > 0);

        // Verify the change was recorded
        let changes = store.get_batch_changes(&batch.id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].entity_id, "test_artist_001");
        assert_eq!(changes[0].operation, ChangeOperation::Create);
    }

    #[test]
    fn test_record_change_auto_creates_batch() {
        let store = create_test_store();

        // Initially no batch exists
        assert!(store.get_active_batch().unwrap().is_none());

        let snapshot = serde_json::json!({"id": "test_artist_001", "name": "The Beatles"});
        let diff = serde_json::json!({});

        // Record a change - should auto-create a batch
        let result = store.record_change(
            ChangeEntityType::Artist,
            "test_artist_001",
            ChangeOperation::Create,
            &diff,
            &snapshot,
            None,
        );

        assert!(result.is_ok());

        // Verify batch was auto-created
        let active_batch = store.get_active_batch().unwrap();
        assert!(active_batch.is_some());
        let batch = active_batch.unwrap();
        // Batch name should be date-based (YYYY-MM-DD format)
        assert!(batch.name.len() == 10);
        assert!(batch.name.chars().nth(4) == Some('-'));
        assert!(batch.name.chars().nth(7) == Some('-'));
    }

    #[test]
    fn test_record_change_updates_batch_activity() {
        let store = create_test_store();

        let batch = store.create_batch("Test Batch", None).unwrap();
        let initial_activity = batch.last_activity_at;

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Record a change
        let snapshot = serde_json::json!({"id": "test_artist_001"});
        store
            .record_change(
                ChangeEntityType::Artist,
                "test_artist_001",
                ChangeOperation::Create,
                &serde_json::json!({}),
                &snapshot,
                None,
            )
            .unwrap();

        // Check that last_activity_at was updated
        let updated_batch = store.get_batch(&batch.id).unwrap().unwrap();
        assert!(updated_batch.last_activity_at >= initial_activity);
    }

    #[test]
    fn test_record_multiple_changes() {
        let store = create_test_store();

        store.create_batch("Multi-change Batch", None).unwrap();

        // Record multiple changes
        for i in 1..=5 {
            let snapshot =
                serde_json::json!({"id": format!("R{}", i), "name": format!("Artist {}", i)});
            store
                .record_change(
                    ChangeEntityType::Artist,
                    &format!("R{}", i),
                    ChangeOperation::Create,
                    &serde_json::json!({}),
                    &snapshot,
                    None,
                )
                .unwrap();
        }

        let batch = store.get_active_batch().unwrap().unwrap();
        assert_eq!(store.get_batch_change_count(&batch.id).unwrap(), 5);
    }

    // =========================================================================
    // Stale batch tests
    // =========================================================================

    #[test]
    fn test_get_stale_batches_empty() {
        let store = create_test_store();

        // No batches at all
        let stale = store.get_stale_batches(24).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_batches_recent_batch_not_stale() {
        let store = create_test_store();

        // Create a batch (it will be "recent" since we just created it)
        store.create_batch("Recent Batch", None).unwrap();

        // With a 24-hour threshold, the just-created batch shouldn't be stale
        let stale = store.get_stale_batches(24).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_batches_closed_batch_not_included() {
        let store = create_test_store();

        // Create and close a batch
        let batch = store.create_batch("Closed Batch", None).unwrap();
        store.close_batch(&batch.id).unwrap();

        // Closed batches should not appear in stale list regardless of threshold
        let stale = store.get_stale_batches(0).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_batches_returns_open_batch_older_than_threshold() {
        let store = create_test_store();

        // Create an open batch
        let batch = store.create_batch("Open Batch", None).unwrap();

        // Manually update created_at to be older than the threshold
        // Simulating a batch created 48 hours ago
        {
            let conn = store.conn.lock().unwrap();
            let old_time = ChangeLogStore::now() - (48 * 3600);
            conn.execute(
                "UPDATE catalog_batches SET created_at = ?1 WHERE id = ?2",
                params![old_time, batch.id],
            )
            .unwrap();
        }

        // With 24-hour threshold, the 48-hour-old batch should be stale
        let stale = store.get_stale_batches(24).unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, batch.id);
        assert_eq!(stale[0].name, "Open Batch");
    }

    // =========================================================================
    // Auto-batch and stale batch tests
    // =========================================================================

    #[test]
    fn test_close_stale_batches_closes_inactive_batch() {
        // Create store with 1 second threshold for testing
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        CATALOG_VERSIONED_SCHEMAS[2].create(&conn).unwrap();
        let store = ChangeLogStore::with_inactivity_threshold(Arc::new(Mutex::new(conn)), 1);

        // Create a batch
        let batch = store.create_batch("Test Batch", None).unwrap();
        assert!(store.get_active_batch().unwrap().is_some());

        // Manually backdate the last_activity_at to make it stale
        {
            let conn = store.conn.lock().unwrap();
            let old_time = ChangeLogStore::now() - 10; // 10 seconds ago
            conn.execute(
                "UPDATE catalog_batches SET last_activity_at = ?1 WHERE id = ?2",
                params![old_time, batch.id],
            )
            .unwrap();
        }

        // Close stale batches
        let closed_count = store.close_stale_batches().unwrap();
        assert_eq!(closed_count, 1);

        // Batch should now be closed
        let active = store.get_active_batch().unwrap();
        assert!(active.is_none());

        // Verify it's closed, not deleted
        let closed_batch = store.get_batch(&batch.id).unwrap().unwrap();
        assert!(!closed_batch.is_open);
        assert!(closed_batch.closed_at.is_some());
    }

    #[test]
    fn test_close_stale_batches_skips_active_batch() {
        // Create store with 1 hour threshold (default)
        let store = create_test_store();

        // Create a batch (it's fresh, not stale)
        store.create_batch("Active Batch", None).unwrap();

        // Try to close stale batches
        let closed_count = store.close_stale_batches().unwrap();
        assert_eq!(closed_count, 0);

        // Batch should still be open
        let active = store.get_active_batch().unwrap();
        assert!(active.is_some());
    }

    #[test]
    fn test_ensure_active_batch_creates_new_when_none_exists() {
        let store = create_test_store();

        // No batch exists
        assert!(store.get_active_batch().unwrap().is_none());

        // Ensure active batch
        let conn = store.conn.lock().unwrap();
        let batch = store.ensure_active_batch_internal(&conn).unwrap();

        // Batch was created with date-based name
        assert!(batch.is_open);
        assert!(batch.name.len() == 10); // YYYY-MM-DD
    }

    #[test]
    fn test_ensure_active_batch_returns_existing_non_stale_batch() {
        let store = create_test_store();

        // Create a batch manually
        let created_batch = store.create_batch("My Batch", None).unwrap();

        // Ensure active batch should return the same one
        let conn = store.conn.lock().unwrap();
        let batch = store.ensure_active_batch_internal(&conn).unwrap();

        assert_eq!(batch.id, created_batch.id);
        assert_eq!(batch.name, "My Batch");
    }

    #[test]
    fn test_ensure_active_batch_closes_stale_and_creates_new() {
        // Create store with 1 second threshold for testing
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        CATALOG_VERSIONED_SCHEMAS[2].create(&conn).unwrap();
        let store = ChangeLogStore::with_inactivity_threshold(Arc::new(Mutex::new(conn)), 1);

        // Create a batch
        let old_batch = store.create_batch("Old Batch", None).unwrap();

        // Manually backdate the last_activity_at to make it stale
        {
            let conn = store.conn.lock().unwrap();
            let old_time = ChangeLogStore::now() - 10; // 10 seconds ago
            conn.execute(
                "UPDATE catalog_batches SET last_activity_at = ?1 WHERE id = ?2",
                params![old_time, old_batch.id],
            )
            .unwrap();
        }

        // Ensure active batch should close old one and create new
        let conn = store.conn.lock().unwrap();
        let new_batch = store.ensure_active_batch_internal(&conn).unwrap();

        // Should be a different batch
        assert_ne!(new_batch.id, old_batch.id);
        // New batch should have date-based name
        assert!(new_batch.name.len() == 10); // YYYY-MM-DD
        drop(conn);

        // Old batch should be closed
        let old_batch_now = store.get_batch(&old_batch.id).unwrap().unwrap();
        assert!(!old_batch_now.is_open);
        assert!(old_batch_now.closed_at.is_some());
    }
}
