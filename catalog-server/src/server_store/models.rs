use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Catalog Events
// =============================================================================

/// Type of catalog event that occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogEventType {
    /// Album metadata or availability changed.
    AlbumUpdated,
    /// Artist metadata changed.
    ArtistUpdated,
    /// Track metadata or availability changed.
    TrackUpdated,
    /// New album added to catalog.
    AlbumAdded,
    /// New artist added.
    ArtistAdded,
    /// New track added.
    TrackAdded,
}

impl CatalogEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatalogEventType::AlbumUpdated => "album_updated",
            CatalogEventType::ArtistUpdated => "artist_updated",
            CatalogEventType::TrackUpdated => "track_updated",
            CatalogEventType::AlbumAdded => "album_added",
            CatalogEventType::ArtistAdded => "artist_added",
            CatalogEventType::TrackAdded => "track_added",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "album_updated" => Some(CatalogEventType::AlbumUpdated),
            "artist_updated" => Some(CatalogEventType::ArtistUpdated),
            "track_updated" => Some(CatalogEventType::TrackUpdated),
            "album_added" => Some(CatalogEventType::AlbumAdded),
            "artist_added" => Some(CatalogEventType::ArtistAdded),
            "track_added" => Some(CatalogEventType::TrackAdded),
            _ => None,
        }
    }
}

/// Content type for catalog events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogContentType {
    Album,
    Artist,
    Track,
}

impl CatalogContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatalogContentType::Album => "album",
            CatalogContentType::Artist => "artist",
            CatalogContentType::Track => "track",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "album" => Some(CatalogContentType::Album),
            "artist" => Some(CatalogContentType::Artist),
            "track" => Some(CatalogContentType::Track),
            _ => None,
        }
    }
}

/// A catalog invalidation event.
///
/// Represents a change to catalog content that clients should respond to
/// by invalidating their cached data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEvent {
    /// Sequence number for ordering and catch-up.
    pub seq: i64,
    /// Type of event.
    pub event_type: CatalogEventType,
    /// Type of content affected.
    pub content_type: CatalogContentType,
    /// ID of the affected content.
    pub content_id: String,
    /// Unix timestamp when the event occurred.
    pub timestamp: i64,
    /// What triggered this event (e.g., "download_completion", "ingestion", "admin_edit").
    pub triggered_by: Option<String>,
}

/// Size limits for bug report fields (in bytes)
pub const BUG_REPORT_TITLE_MAX_LEN: usize = 200;
pub const BUG_REPORT_DESCRIPTION_MAX_SIZE: usize = 100 * 1024; // 100KB
pub const BUG_REPORT_LOGS_MAX_SIZE: usize = 1024 * 1024; // 1MB
pub const BUG_REPORT_ATTACHMENT_MAX_SIZE: usize = 25 * 1024 * 1024; // 25MB per image
pub const BUG_REPORT_MAX_ATTACHMENTS: usize = 5;
pub const BUG_REPORT_TOTAL_MAX_SIZE: usize = 500 * 1024 * 1024; // 500MB total storage

/// A bug report submitted by a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugReport {
    pub id: String,
    pub user_id: usize,
    pub user_handle: String,
    pub title: Option<String>,
    pub description: String,
    pub client_type: String,
    pub client_version: Option<String>,
    pub device_info: Option<String>,
    pub logs: Option<String>,
    /// JSON array of base64-encoded images
    pub attachments: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Summary of a bug report for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugReportSummary {
    pub id: String,
    pub user_id: usize,
    pub user_handle: String,
    pub title: Option<String>,
    pub client_type: String,
    pub created_at: DateTime<Utc>,
    /// Approximate size in bytes (description + logs + attachments)
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunStatus {
    Running,
    Completed,
    Failed,
}

/// Event types for job audit log entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobAuditEventType {
    Started,
    Completed,
    Failed,
    Progress,
}

impl JobAuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobAuditEventType::Started => "started",
            JobAuditEventType::Completed => "completed",
            JobAuditEventType::Failed => "failed",
            JobAuditEventType::Progress => "progress",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "started" => Some(JobAuditEventType::Started),
            "completed" => Some(JobAuditEventType::Completed),
            "failed" => Some(JobAuditEventType::Failed),
            "progress" => Some(JobAuditEventType::Progress),
            _ => None,
        }
    }
}

/// An entry in the job audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAuditEntry {
    pub id: i64,
    pub job_id: String,
    pub event_type: JobAuditEventType,
    /// Unix timestamp when the event occurred
    pub timestamp: i64,
    pub duration_ms: Option<i64>,
    pub details: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl JobRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobRunStatus::Running => "running",
            JobRunStatus::Completed => "completed",
            JobRunStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(JobRunStatus::Running),
            "completed" => Some(JobRunStatus::Completed),
            "failed" => Some(JobRunStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JobRun {
    pub id: i64,
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: JobRunStatus,
    pub error_message: Option<String>,
    /// How the job was triggered: "schedule", "hook:OnStartup", "manual", etc.
    pub triggered_by: String,
}

#[derive(Debug, Clone)]
pub struct JobScheduleState {
    pub job_id: String,
    pub next_run_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
}
