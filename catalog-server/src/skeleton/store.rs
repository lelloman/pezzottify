//! Skeleton event store implementation.

use super::models::{SkeletonEvent, SkeletonEventType};
use super::schema::SKELETON_VERSIONED_SCHEMAS;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Store for skeleton events and metadata.
#[derive(Clone)]
pub struct SkeletonEventStore {
    conn: Arc<Mutex<Connection>>,
}

impl SkeletonEventStore {
    /// Create a new SkeletonEventStore with the given database connection.
    ///
    /// This will initialize the schema if the tables don't exist.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        {
            let conn = conn.lock().unwrap();
            // Initialize schema
            let schema = SKELETON_VERSIONED_SCHEMAS.first().unwrap();
            conn.execute_batch(schema.up)
                .context("Failed to initialize skeleton schema")?;

            // Initialize catalog_version if not present
            conn.execute(
                "INSERT OR IGNORE INTO catalog_meta (key, value) VALUES ('catalog_version', '0')",
                [],
            )?;
        }

        Ok(Self { conn })
    }

    /// Get the current catalog version.
    pub fn get_version(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM catalog_meta WHERE key = 'catalog_version'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        Ok(version.parse().unwrap_or(0))
    }

    /// Get the cached checksum.
    pub fn get_checksum(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT value FROM catalog_meta WHERE key = 'catalog_checksum'",
            [],
            |row| row.get(0),
        ) {
            Ok(checksum) => Ok(Some(checksum)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set the cached checksum.
    pub fn set_checksum(&self, checksum: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO catalog_meta (key, value) VALUES ('catalog_checksum', ?1)",
            params![checksum],
        )?;
        Ok(())
    }

    /// Emit a skeleton event and increment the catalog version.
    ///
    /// Returns the new catalog version.
    pub fn emit_event(
        &self,
        event_type: SkeletonEventType,
        entity_id: &str,
        payload: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO catalog_events (event_type, entity_id, payload, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![event_type.as_str(), entity_id, payload, timestamp],
        )?;

        // Increment version
        conn.execute(
            "UPDATE catalog_meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'catalog_version'",
            [],
        )?;

        // Invalidate checksum (will be recalculated on next request)
        conn.execute(
            "DELETE FROM catalog_meta WHERE key = 'catalog_checksum'",
            [],
        )?;

        let version: i64 = conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM catalog_meta WHERE key = 'catalog_version'",
            [],
            |row| row.get(0),
        )?;

        Ok(version)
    }

    /// Get all events since the given sequence number (exclusive).
    pub fn get_events_since(&self, seq: i64) -> Result<Vec<SkeletonEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT seq, event_type, entity_id, payload, timestamp
             FROM catalog_events
             WHERE seq > ?1
             ORDER BY seq ASC",
        )?;

        let events = stmt
            .query_map(params![seq], |row| {
                let event_type_str: String = row.get(1)?;
                let event_type = SkeletonEventType::from_str(&event_type_str)
                    .unwrap_or(SkeletonEventType::ArtistAdded);

                Ok(SkeletonEvent {
                    seq: row.get(0)?,
                    event_type,
                    entity_id: row.get(2)?,
                    payload: row.get(3)?,
                    timestamp: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get the earliest available sequence number.
    ///
    /// Returns 0 if no events exist.
    pub fn get_earliest_seq(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row("SELECT MIN(seq) FROM catalog_events", [], |row| {
            row.get::<_, Option<i64>>(0)
        }) {
            Ok(Some(seq)) => Ok(seq),
            Ok(None) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the latest sequence number.
    ///
    /// Returns 0 if no events exist.
    pub fn get_latest_seq(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row("SELECT MAX(seq) FROM catalog_events", [], |row| {
            row.get::<_, Option<i64>>(0)
        }) {
            Ok(Some(seq)) => Ok(seq),
            Ok(None) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    /// Calculate and set the checksum based on catalog IDs.
    ///
    /// The checksum is SHA256 of all sorted IDs concatenated together.
    /// The IDs should be pre-sorted by the caller.
    pub fn calculate_and_set_checksum(
        &self,
        artist_ids: &[String],
        album_ids: &[String],
        track_ids: &[String],
    ) -> Result<String> {
        let mut hasher = Sha256::new();

        // Hash all artist IDs (should be sorted)
        for id in artist_ids {
            hasher.update(id.as_bytes());
            hasher.update(b"\n");
        }

        // Hash all album IDs (should be sorted)
        for id in album_ids {
            hasher.update(id.as_bytes());
            hasher.update(b"\n");
        }

        // Hash all track IDs (should be sorted)
        for id in track_ids {
            hasher.update(id.as_bytes());
            hasher.update(b"\n");
        }

        let result = hasher.finalize();
        let checksum = format!("sha256:{:x}", result);

        // Cache the checksum
        self.set_checksum(&checksum)?;

        Ok(checksum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_store() -> SkeletonEventStore {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        SkeletonEventStore::new(conn).unwrap()
    }

    #[test]
    fn test_initial_version_is_zero() {
        let store = create_test_store();
        assert_eq!(store.get_version().unwrap(), 0);
    }

    #[test]
    fn test_emit_event_increments_version() {
        let store = create_test_store();

        let version = store
            .emit_event(SkeletonEventType::ArtistAdded, "artist1", None)
            .unwrap();
        assert_eq!(version, 1);

        let version = store
            .emit_event(SkeletonEventType::AlbumAdded, "album1", Some(r#"{"artist_ids":["artist1"]}"#))
            .unwrap();
        assert_eq!(version, 2);

        assert_eq!(store.get_version().unwrap(), 2);
    }

    #[test]
    fn test_get_events_since() {
        let store = create_test_store();

        store
            .emit_event(SkeletonEventType::ArtistAdded, "artist1", None)
            .unwrap();
        store
            .emit_event(SkeletonEventType::ArtistAdded, "artist2", None)
            .unwrap();
        store
            .emit_event(SkeletonEventType::AlbumAdded, "album1", Some(r#"{"artist_ids":["artist1"]}"#))
            .unwrap();

        let events = store.get_events_since(0).unwrap();
        assert_eq!(events.len(), 3);

        let events = store.get_events_since(1).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].entity_id, "artist2");

        let events = store.get_events_since(3).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_checksum_management() {
        let store = create_test_store();

        assert!(store.get_checksum().unwrap().is_none());

        store.set_checksum("sha256:abc123").unwrap();
        assert_eq!(store.get_checksum().unwrap(), Some("sha256:abc123".to_string()));

        // Emit event should invalidate checksum
        store
            .emit_event(SkeletonEventType::ArtistAdded, "artist1", None)
            .unwrap();
        assert!(store.get_checksum().unwrap().is_none());
    }

    #[test]
    fn test_earliest_and_latest_seq() {
        let store = create_test_store();

        // No events yet
        assert_eq!(store.get_earliest_seq().unwrap(), 0);
        assert_eq!(store.get_latest_seq().unwrap(), 0);

        store
            .emit_event(SkeletonEventType::ArtistAdded, "artist1", None)
            .unwrap();
        store
            .emit_event(SkeletonEventType::ArtistAdded, "artist2", None)
            .unwrap();

        assert_eq!(store.get_earliest_seq().unwrap(), 1);
        assert_eq!(store.get_latest_seq().unwrap(), 2);
    }

    #[test]
    fn test_calculate_and_set_checksum() {
        let store = create_test_store();

        let artist_ids = vec!["artist1".to_string(), "artist2".to_string()];
        let album_ids = vec!["album1".to_string()];
        let track_ids = vec!["track1".to_string(), "track2".to_string(), "track3".to_string()];

        let checksum = store
            .calculate_and_set_checksum(&artist_ids, &album_ids, &track_ids)
            .unwrap();

        // Checksum should start with sha256:
        assert!(checksum.starts_with("sha256:"));
        assert!(checksum.len() > 10); // Should have actual hash content

        // Should be stored
        assert_eq!(store.get_checksum().unwrap(), Some(checksum.clone()));

        // Same inputs should produce same checksum
        let checksum2 = store
            .calculate_and_set_checksum(&artist_ids, &album_ids, &track_ids)
            .unwrap();
        assert_eq!(checksum, checksum2);

        // Different inputs should produce different checksum
        let different_checksum = store
            .calculate_and_set_checksum(&["other".to_string()], &[], &[])
            .unwrap();
        assert_ne!(checksum, different_checksum);
    }
}
