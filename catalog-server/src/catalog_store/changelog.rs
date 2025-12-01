//! Catalog changelog models and types.
//!
//! This module defines the types for tracking changes to catalog entities
//! (artists, albums, tracks, images) through batched changelog entries.

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
