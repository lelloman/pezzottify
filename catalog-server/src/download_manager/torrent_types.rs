//! Quentin Torrentino API types.
//!
//! Defines ticket structures, API responses, and WebSocket messages
//! for communication with the Quentin Torrentino service.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

// =============================================================================
// Ticket Request Types (sent to QT)
// =============================================================================

/// Request body for creating a ticket (matches QT's CreateTicketBody).
#[derive(Debug, Clone, Serialize)]
pub struct CreateTicketRequest {
    /// Priority for queue ordering (higher = more urgent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    /// Query context for search/matching
    pub query_context: QueryContext,
    /// Destination path for final output
    pub dest_path: String,
    /// Output format constraints (None = keep original, no conversion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_constraints: Option<OutputConstraints>,
}

/// Query context for ticket search/matching.
#[derive(Debug, Clone, Serialize)]
pub struct QueryContext {
    /// Structured tags for categorization (e.g., ["music", "flac"])
    pub tags: Vec<String>,
    /// Freeform description for matching (e.g., "Pink Floyd Dark Side of the Moon")
    pub description: String,
    /// Expected content (tracks, episodes, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<ExpectedContent>,
    /// Search constraints (preferred formats, quality, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_constraints: Option<SearchConstraints>,
}

/// Expected content in a ticket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedContent {
    /// An album with tracks
    Album {
        /// Artist name (optional for various artists compilations)
        artist: Option<String>,
        /// Album title
        title: String,
        /// Expected tracks
        tracks: Vec<ExpectedTrack>,
    },
    /// A single track
    Track {
        /// Artist name
        artist: Option<String>,
        /// Track title
        title: String,
    },
}

/// Expected track in an album.
#[derive(Debug, Clone, Serialize)]
pub struct ExpectedTrack {
    /// Track number (1-indexed)
    pub number: u32,
    /// Track title
    pub title: String,
    /// Duration in seconds (optional, for validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u32>,
    /// Disc number (for multi-disc albums)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
}

/// Search constraints for ticket matching.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchConstraints {
    /// Audio-specific constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioSearchConstraints>,
}

/// Audio format specification (matches QT's AudioFormat enum).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Flac,
    Mp3,
    Aac,
    OggVorbis,
    Opus,
    Wav,
    Alac,
}

/// Audio-specific search constraints.
#[derive(Debug, Clone, Serialize, Default)]
pub struct AudioSearchConstraints {
    /// Preferred audio formats (ordered by preference)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_formats: Vec<AudioFormat>,
    /// Minimum bitrate in kbps (for lossy formats)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_bitrate_kbps: Option<u32>,
    /// Avoid compilation/various artists releases
    #[serde(default)]
    pub avoid_compilations: bool,
    /// Avoid live recordings
    #[serde(default)]
    pub avoid_live: bool,
}

/// Constraints for audio conversion (matches QT's AudioConstraints).
#[derive(Debug, Clone, Serialize)]
pub struct AudioConstraints {
    /// Target audio format
    pub format: AudioFormat,
    /// Target bitrate in kbps (for lossy formats)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    /// Target sample rate in Hz
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
}

impl Default for AudioConstraints {
    fn default() -> Self {
        Self {
            format: AudioFormat::OggVorbis,
            bitrate_kbps: Some(320),
            sample_rate_hz: None, // Keep original
        }
    }
}

/// Output format constraints for conversion (matches QT's OutputConstraints enum).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputConstraints {
    /// Keep original format - no conversion
    Original,
    /// Convert audio files to specified format
    Audio(AudioConstraints),
}

impl Default for OutputConstraints {
    fn default() -> Self {
        Self::Audio(AudioConstraints::default())
    }
}

// =============================================================================
// Ticket Response Types (received from QT)
// =============================================================================

/// Response when creating a ticket.
#[derive(Debug, Clone, Deserialize)]
pub struct TicketResponse {
    /// Ticket ID assigned by QT
    pub id: String,
    /// Creation timestamp
    pub created_at: String,
    /// Creator ID
    pub created_by: String,
    /// Current state
    pub state: TicketStateResponse,
    /// Priority
    pub priority: u16,
    /// Query context
    pub query_context: serde_json::Value,
    /// Destination path
    pub dest_path: String,
    /// Output constraints
    pub output_constraints: Option<serde_json::Value>,
    /// Last update timestamp
    pub updated_at: String,
}

/// Ticket state response from QT.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TicketStateResponse {
    Pending,
    Searching,
    Matching,
    NeedsApproval { candidates: Vec<serde_json::Value> },
    Approved { selected: serde_json::Value },
    Downloading { progress: f32 },
    Converting { progress: f32 },
    Placing,
    Completed { completed_at: String },
    Failed { error: String, retryable: bool },
    AcquisitionFailed { error: String, retryable: bool },
    Cancelled { reason: Option<String> },
    Rejected { reason: Option<String> },
}

/// Current state of a ticket (simplified).
#[derive(Debug, Clone, Deserialize)]
pub struct TicketState {
    /// Ticket ID
    #[serde(alias = "id")]
    pub ticket_id: String,
    /// Current state type
    #[serde(flatten)]
    pub state: TicketStateResponse,
}

/// QT statistics response.
#[derive(Debug, Clone, Deserialize)]
pub struct QTStats {
    pub pending: Option<i64>,
    pub downloading: Option<i64>,
    pub completed_today: Option<i64>,
}

// =============================================================================
// WebSocket Message Types
// =============================================================================

/// WebSocket message received from Quentin Torrentino.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TorrentEvent {
    /// Ticket changed state (QT sends "ticket_update", we also accept "ticket_updated")
    #[serde(alias = "ticket_update")]
    TicketUpdated { ticket_id: String, state: String },
    /// Ticket deleted
    TicketDeleted { ticket_id: String },
    /// Download progress update
    Progress {
        ticket_id: String,
        state: String,
        progress_pct: f32,
        speed_bps: Option<u64>,
        eta_secs: Option<u64>,
    },
    /// Ticket needs manual approval (multiple torrent candidates)
    NeedsApproval {
        ticket_id: String,
        candidates: Vec<TorrentCandidate>,
    },
    /// Ticket completed successfully
    Completed {
        ticket_id: String,
        items_placed: u32,
    },
    /// Ticket failed
    Failed {
        ticket_id: String,
        error: String,
        retryable: bool,
    },
}

/// A torrent candidate for manual selection.
#[derive(Debug, Clone, Deserialize)]
pub struct TorrentCandidate {
    /// Candidate title/name
    pub title: String,
    /// Match score (0.0-1.0)
    pub score: f32,
    /// Number of seeders
    pub seeders: u32,
    /// Size in bytes
    pub size_bytes: Option<u64>,
}

// =============================================================================
// Ticket Status Enum (mirrors QT states)
// =============================================================================

/// Ticket status states from Quentin Torrentino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    /// Initial state, waiting to be processed
    Pending,
    /// Searching for matching torrents
    Searching,
    /// Matching found torrents to tracks
    Matching,
    /// Waiting for admin approval (multiple candidates)
    NeedsApproval,
    /// Approved for download
    Approved,
    /// Downloading torrent
    Downloading,
    /// Converting audio format
    Converting,
    /// Placing files in destination
    Placing,
    /// Successfully completed
    Completed,
    /// Search found no results
    SearchFailed,
    /// Rejected by admin
    Rejected,
    /// Failed (may be retryable)
    Failed,
    /// Acquisition failed
    AcquisitionFailed,
    /// Cancelled
    Cancelled,
}

/// Error type for parsing TicketStatus from string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTicketStatusError(pub String);

impl std::fmt::Display for ParseTicketStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown ticket status: {}", self.0)
    }
}

impl std::error::Error for ParseTicketStatusError {}

impl FromStr for TicketStatus {
    type Err = ParseTicketStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "searching" => Ok(Self::Searching),
            "matching" => Ok(Self::Matching),
            "needs_approval" => Ok(Self::NeedsApproval),
            "approved" | "auto_approved" => Ok(Self::Approved),
            "downloading" => Ok(Self::Downloading),
            "converting" => Ok(Self::Converting),
            "placing" => Ok(Self::Placing),
            "completed" => Ok(Self::Completed),
            "search_failed" => Ok(Self::SearchFailed),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            "acquisition_failed" => Ok(Self::AcquisitionFailed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(ParseTicketStatusError(s.to_string())),
        }
    }
}

impl TicketStatus {
    /// Convert to database string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Searching => "searching",
            Self::Matching => "matching",
            Self::NeedsApproval => "needs_approval",
            Self::Approved => "approved",
            Self::Downloading => "downloading",
            Self::Converting => "converting",
            Self::Placing => "placing",
            Self::Completed => "completed",
            Self::SearchFailed => "search_failed",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::AcquisitionFailed => "acquisition_failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::SearchFailed
                | Self::Rejected
                | Self::Failed
                | Self::AcquisitionFailed
                | Self::Cancelled
        )
    }

    /// Returns true if this state indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed)
    }

    /// Returns true if this state indicates an error.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Self::SearchFailed | Self::Rejected | Self::Failed | Self::AcquisitionFailed
        )
    }
}

// =============================================================================
// Ticket Mapping (local DB)
// =============================================================================

/// Mapping between local queue item and QT ticket.
#[derive(Debug, Clone, Serialize)]
pub struct TicketMapping {
    /// Local queue item ID
    pub queue_item_id: String,
    /// QT ticket ID
    pub ticket_id: String,
    /// Current ticket state (from QT)
    pub ticket_state: TicketStatus,
    /// Album ID this ticket is for
    pub album_id: String,
    /// When the ticket was created
    pub created_at: i64,
    /// When the ticket state was last updated
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticket_status_from_str() {
        assert_eq!(TicketStatus::from_str("pending"), Ok(TicketStatus::Pending));
        assert_eq!(
            TicketStatus::from_str("downloading"),
            Ok(TicketStatus::Downloading)
        );
        assert_eq!(
            TicketStatus::from_str("auto_approved"),
            Ok(TicketStatus::Approved)
        );
        assert!(TicketStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_ticket_status_terminal() {
        assert!(!TicketStatus::Pending.is_terminal());
        assert!(!TicketStatus::Downloading.is_terminal());
        assert!(TicketStatus::Completed.is_terminal());
        assert!(TicketStatus::Failed.is_terminal());
        assert!(TicketStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_output_constraints_default() {
        let constraints = OutputConstraints::default();
        match constraints {
            OutputConstraints::Audio(audio) => {
                assert!(matches!(audio.format, AudioFormat::OggVorbis));
                assert_eq!(audio.bitrate_kbps, Some(320));
            }
            _ => panic!("Expected Audio variant"),
        }
    }

    #[test]
    fn test_torrent_event_deserialize() {
        let json = r#"{"type": "completed", "ticket_id": "abc123", "items_placed": 12}"#;
        let event: TorrentEvent = serde_json::from_str(json).unwrap();
        match event {
            TorrentEvent::Completed {
                ticket_id,
                items_placed,
            } => {
                assert_eq!(ticket_id, "abc123");
                assert_eq!(items_placed, 12);
            }
            _ => panic!("Expected Completed event"),
        }
    }

    #[test]
    fn test_torrent_event_failed_deserialize() {
        let json = r#"{"type": "failed", "ticket_id": "abc123", "error": "No torrents found", "retryable": true}"#;
        let event: TorrentEvent = serde_json::from_str(json).unwrap();
        match event {
            TorrentEvent::Failed {
                ticket_id,
                error,
                retryable,
            } => {
                assert_eq!(ticket_id, "abc123");
                assert_eq!(error, "No torrents found");
                assert!(retryable);
            }
            _ => panic!("Expected Failed event"),
        }
    }

    #[test]
    fn test_create_ticket_request_serialization() {
        let request = CreateTicketRequest {
            priority: Some(10),
            query_context: QueryContext {
                tags: vec!["music".to_string(), "flac".to_string()],
                description: "Pink Floyd Dark Side of the Moon".to_string(),
                expected: Some(ExpectedContent::Album {
                    artist: Some("Pink Floyd".to_string()),
                    title: "Dark Side of the Moon".to_string(),
                    tracks: vec![ExpectedTrack {
                        number: 1,
                        title: "Speak to Me".to_string(),
                        duration_secs: Some(90),
                        disc_number: Some(1),
                    }],
                }),
                search_constraints: None,
            },
            dest_path: "/media/music".to_string(),
            output_constraints: Some(OutputConstraints::default()),
        };

        let json = serde_json::to_string_pretty(&request).unwrap();
        assert!(json.contains("Pink Floyd"));
        assert!(json.contains("Dark Side of the Moon"));
        assert!(json.contains("Speak to Me"));
    }
}
