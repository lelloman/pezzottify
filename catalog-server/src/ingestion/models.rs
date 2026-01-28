//! Data models for the ingestion feature.
//!
//! Album-first ingestion workflow:
//! 1. User uploads a zip/folder containing audio files
//! 2. System extracts and analyzes all files (embedded tags, duration)
//! 3. Agent identifies the album from collective metadata
//! 4. Once album is confirmed, files are mapped to tracks
//! 5. Matched files are converted and stored

use serde::{Deserialize, Serialize};

/// Status of an ingestion job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IngestionJobStatus {
    /// Job created, waiting to be processed.
    Pending,
    /// Extracting and analyzing audio files.
    Analyzing,
    /// Agent is identifying the album.
    IdentifyingAlbum,
    /// Waiting for human review to confirm album.
    AwaitingReview,
    /// Mapping files to tracks within the album.
    MappingTracks,
    /// Converting audio files to target format.
    Converting,
    /// Successfully completed.
    Completed,
    /// Failed (non-recoverable).
    Failed,
}

impl IngestionJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Analyzing => "ANALYZING",
            Self::IdentifyingAlbum => "IDENTIFYING_ALBUM",
            Self::AwaitingReview => "AWAITING_REVIEW",
            Self::MappingTracks => "MAPPING_TRACKS",
            Self::Converting => "CONVERTING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PENDING" => Some(Self::Pending),
            "ANALYZING" => Some(Self::Analyzing),
            "IDENTIFYING_ALBUM" => Some(Self::IdentifyingAlbum),
            "AWAITING_REVIEW" => Some(Self::AwaitingReview),
            "MAPPING_TRACKS" => Some(Self::MappingTracks),
            "CONVERTING" => Some(Self::Converting),
            "COMPLETED" => Some(Self::Completed),
            "FAILED" => Some(Self::Failed),
            // Legacy status mappings for backwards compatibility
            "PROCESSING" => Some(Self::IdentifyingAlbum),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

/// Type of ingestion context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IngestionContextType {
    /// Spontaneous upload (no prior request).
    Spontaneous,
    /// Fulfilling a download request.
    DownloadRequest,
}

impl IngestionContextType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spontaneous => "SPONTANEOUS",
            Self::DownloadRequest => "DOWNLOAD_REQUEST",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "SPONTANEOUS" => Some(Self::Spontaneous),
            "DOWNLOAD_REQUEST" => Some(Self::DownloadRequest),
            _ => None,
        }
    }
}

/// Source of the match decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IngestionMatchSource {
    /// Matched by the agent automatically.
    Agent,
    /// Matched via human review.
    HumanReview,
    /// Matched from download request (album was pre-specified).
    DownloadRequest,
    /// Matched via duration fingerprint algorithm.
    Fingerprint,
}

impl IngestionMatchSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "AGENT",
            Self::HumanReview => "HUMAN_REVIEW",
            Self::DownloadRequest => "DOWNLOAD_REQUEST",
            Self::Fingerprint => "FINGERPRINT",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "AGENT" => Some(Self::Agent),
            "HUMAN_REVIEW" => Some(Self::HumanReview),
            "DOWNLOAD_REQUEST" => Some(Self::DownloadRequest),
            "FINGERPRINT" => Some(Self::Fingerprint),
            _ => None,
        }
    }
}

/// Type of upload detected from file structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UploadType {
    /// Single audio file.
    Track,
    /// Multiple audio files representing an album.
    Album,
    /// Multiple directories containing albums.
    Collection,
}

impl UploadType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Track => "TRACK",
            Self::Album => "ALBUM",
            Self::Collection => "COLLECTION",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "TRACK" => Some(Self::Track),
            "ALBUM" => Some(Self::Album),
            "COLLECTION" => Some(Self::Collection),
            _ => None,
        }
    }
}

/// Ticket type based on fingerprint match quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TicketType {
    /// 100% match with delta < 1s - auto-ingest.
    Success,
    /// 90-99% match - needs human review.
    Review,
    /// < 90% match or no candidates - manual resolution required.
    Failure,
}

impl TicketType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Review => "REVIEW",
            Self::Failure => "FAILURE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "SUCCESS" => Some(Self::Success),
            "REVIEW" => Some(Self::Review),
            "FAILURE" => Some(Self::Failure),
            _ => None,
        }
    }
}

/// An ingestion job representing an album upload.
///
/// A job contains multiple files (extracted from a zip or uploaded individually).
/// The workflow identifies the album first, then maps files to tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionJob {
    /// Unique identifier.
    pub id: String,
    /// Current status.
    pub status: IngestionJobStatus,
    /// User who uploaded the file.
    pub user_id: String,
    /// Original filename (zip name or folder name).
    pub original_filename: String,
    /// Total size of all files in bytes.
    pub total_size_bytes: i64,
    /// Number of audio files in this job.
    pub file_count: i32,

    // Context
    /// Type of ingestion context.
    pub context_type: Option<IngestionContextType>,
    /// Context ID (e.g., download_queue_item_id).
    pub context_id: Option<String>,

    // Upload session and type (for collection uploads)
    /// Groups jobs from the same upload (for collections).
    pub upload_session_id: Option<String>,
    /// Detected upload type (Track, Album, Collection).
    pub upload_type: Option<UploadType>,

    // Album identification (populated during IDENTIFYING_ALBUM phase)
    /// Detected artist name (from embedded tags).
    pub detected_artist: Option<String>,
    /// Detected album name (from embedded tags).
    pub detected_album: Option<String>,
    /// Detected year (from embedded tags).
    pub detected_year: Option<i32>,

    // Album match result
    /// Matched album ID (if found).
    pub matched_album_id: Option<String>,
    /// Match confidence (0.0 - 1.0).
    pub match_confidence: Option<f32>,
    /// Source of the match.
    pub match_source: Option<IngestionMatchSource>,

    // Fingerprint match details
    /// Ticket type based on fingerprint match quality.
    pub ticket_type: Option<TicketType>,
    /// Fingerprint match score (percentage of tracks matched).
    pub match_score: Option<f32>,
    /// Total duration delta in milliseconds across all tracks.
    pub match_delta_ms: Option<i64>,

    // Stats
    /// Number of tracks successfully matched.
    pub tracks_matched: i32,
    /// Number of tracks converted.
    pub tracks_converted: i32,

    // Error handling
    /// Error message (if failed).
    pub error_message: Option<String>,

    // Timestamps (Unix millis)
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,

    // Workflow state (JSON blob for resumable workflows)
    pub workflow_state: Option<String>,
}

impl IngestionJob {
    /// Create a new pending ingestion job.
    pub fn new(
        id: impl Into<String>,
        user_id: impl Into<String>,
        original_filename: impl Into<String>,
        total_size_bytes: i64,
        file_count: i32,
    ) -> Self {
        Self {
            id: id.into(),
            status: IngestionJobStatus::Pending,
            user_id: user_id.into(),
            original_filename: original_filename.into(),
            total_size_bytes,
            file_count,
            context_type: None,
            context_id: None,
            upload_session_id: None,
            upload_type: None,
            detected_artist: None,
            detected_album: None,
            detected_year: None,
            matched_album_id: None,
            match_confidence: None,
            match_source: None,
            ticket_type: None,
            match_score: None,
            match_delta_ms: None,
            tracks_matched: 0,
            tracks_converted: 0,
            error_message: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            completed_at: None,
            workflow_state: None,
        }
    }

    /// Set the context for this job.
    pub fn with_context(
        mut self,
        context_type: IngestionContextType,
        context_id: Option<String>,
    ) -> Self {
        self.context_type = Some(context_type);
        self.context_id = context_id;
        self
    }

    /// Set the upload session and type for this job.
    pub fn with_upload_info(mut self, session_id: Option<String>, upload_type: UploadType) -> Self {
        self.upload_session_id = session_id;
        self.upload_type = Some(upload_type);
        self
    }

    /// Set the fingerprint match result for this job.
    pub fn with_fingerprint_match(
        mut self,
        ticket_type: TicketType,
        match_score: f32,
        match_delta_ms: i64,
        matched_album_id: Option<String>,
    ) -> Self {
        self.ticket_type = Some(ticket_type);
        self.match_score = Some(match_score);
        self.match_delta_ms = Some(match_delta_ms);
        self.matched_album_id = matched_album_id;
        self.match_source = Some(IngestionMatchSource::Fingerprint);
        self
    }
}

/// A single audio file within an ingestion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionFile {
    /// Unique identifier.
    pub id: String,
    /// Parent job ID.
    pub job_id: String,
    /// Original filename.
    pub filename: String,
    /// File size in bytes.
    pub file_size_bytes: i64,
    /// Path to temporary file.
    pub temp_file_path: String,

    // Audio metadata (from ffprobe)
    /// Duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Audio codec.
    pub codec: Option<String>,
    /// Bitrate in kbps.
    pub bitrate: Option<i32>,
    /// Sample rate in Hz.
    pub sample_rate: Option<i32>,

    // Embedded tags (from ID3/Vorbis comments)
    /// Artist name from tags.
    pub tag_artist: Option<String>,
    /// Album name from tags.
    pub tag_album: Option<String>,
    /// Track title from tags.
    pub tag_title: Option<String>,
    /// Track number from tags.
    pub tag_track_num: Option<i32>,
    /// Total tracks from tags.
    pub tag_track_total: Option<i32>,
    /// Disc number from tags.
    pub tag_disc_num: Option<i32>,
    /// Year from tags.
    pub tag_year: Option<i32>,

    // Match result (populated during MAPPING_TRACKS phase)
    /// Matched track ID.
    pub matched_track_id: Option<String>,
    /// Match confidence for this file.
    pub match_confidence: Option<f32>,

    // Output
    /// Final output file path (after conversion).
    pub output_file_path: Option<String>,
    /// Whether this file was successfully converted.
    pub converted: bool,
    /// Error message for this specific file.
    pub error_message: Option<String>,

    // Conversion decision
    /// Reason for conversion or why it was skipped.
    pub conversion_reason: Option<ConversionReason>,

    // Timestamps
    pub created_at: i64,
}

impl IngestionFile {
    /// Create a new ingestion file record.
    pub fn new(
        id: impl Into<String>,
        job_id: impl Into<String>,
        filename: impl Into<String>,
        file_size_bytes: i64,
        temp_file_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            job_id: job_id.into(),
            filename: filename.into(),
            file_size_bytes,
            temp_file_path: temp_file_path.into(),
            duration_ms: None,
            codec: None,
            bitrate: None,
            sample_rate: None,
            tag_artist: None,
            tag_album: None,
            tag_title: None,
            tag_track_num: None,
            tag_track_total: None,
            tag_disc_num: None,
            tag_year: None,
            matched_track_id: None,
            match_confidence: None,
            output_file_path: None,
            converted: false,
            error_message: None,
            conversion_reason: None,
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Aggregated metadata from all files in a job.
/// Used by the agent to identify the album.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumMetadataSummary {
    /// Most common artist across files.
    pub artist: Option<String>,
    /// Most common album name across files.
    pub album: Option<String>,
    /// Year (if consistent).
    pub year: Option<i32>,
    /// Number of files.
    pub file_count: i32,
    /// Total duration in milliseconds.
    pub total_duration_ms: i64,
    /// List of track titles (in order by track number if available).
    pub track_titles: Vec<String>,
    /// Original zip/folder name.
    pub source_name: String,
}

/// An item in the human review queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueueItem {
    /// Auto-increment ID.
    pub id: i64,
    /// Associated job ID.
    pub job_id: String,
    /// Question to present to the reviewer.
    pub question: String,
    /// Options as JSON array.
    pub options: String,
    /// When the item was created.
    pub created_at: i64,
    /// When it was resolved (if resolved).
    pub resolved_at: Option<i64>,
    /// User who resolved it.
    pub resolved_by_user_id: Option<String>,
    /// Selected option.
    pub selected_option: Option<String>,
}

/// Option for human review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewOption {
    /// Unique ID for this option (e.g., "album:abc123").
    pub id: String,
    /// Display label.
    pub label: String,
    /// Additional description.
    pub description: Option<String>,
}

/// Reason why a file needs conversion (or why it was skipped).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConversionReason {
    /// Bitrate within target range, format acceptable - no conversion needed
    NoConversionNeeded,
    /// Bitrate too high, downsampling to target bitrate
    HighBitrate { original_bitrate: i32 },
    /// Bitrate too low, awaiting user confirmation
    LowBitratePendingConfirmation { original_bitrate: i32 },
    /// User approved conversion of low bitrate file
    LowBitrateApproved { original_bitrate: i32 },
    /// Could not detect bitrate - will convert to ensure known quality
    UndetectableBitrate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion_reason_serialization() {
        // Test NoConversionNeeded - simple enum variant serializes as a string
        let reason = ConversionReason::NoConversionNeeded;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#""NO_CONVERSION_NEEDED""#);

        let deserialized: ConversionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ConversionReason::NoConversionNeeded));

        // Test HighBitrate - enum with data serializes as object with variant name as key
        let reason = ConversionReason::HighBitrate {
            original_bitrate: 500,
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#"{"HIGH_BITRATE":{"original_bitrate":500}}"#);

        let deserialized: ConversionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ConversionReason::HighBitrate {
                original_bitrate: 500
            }
        ));

        // Test LowBitratePendingConfirmation
        let reason = ConversionReason::LowBitratePendingConfirmation {
            original_bitrate: 128,
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(
            json,
            r#"{"LOW_BITRATE_PENDING_CONFIRMATION":{"original_bitrate":128}}"#
        );

        let deserialized: ConversionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: 128
            }
        ));

        // Test LowBitrateApproved
        let reason = ConversionReason::LowBitrateApproved {
            original_bitrate: 192,
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#"{"LOW_BITRATE_APPROVED":{"original_bitrate":192}}"#);

        let deserialized: ConversionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ConversionReason::LowBitrateApproved {
                original_bitrate: 192
            }
        ));

        // Test UndetectableBitrate
        let reason = ConversionReason::UndetectableBitrate;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#""UNDETECTABLE_BITRATE""#);

        let deserialized: ConversionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ConversionReason::UndetectableBitrate
        ));
    }

    #[test]
    fn test_job_status_roundtrip() {
        for status in [
            IngestionJobStatus::Pending,
            IngestionJobStatus::Analyzing,
            IngestionJobStatus::IdentifyingAlbum,
            IngestionJobStatus::AwaitingReview,
            IngestionJobStatus::MappingTracks,
            IngestionJobStatus::Converting,
            IngestionJobStatus::Completed,
            IngestionJobStatus::Failed,
        ] {
            let s = status.as_str();
            let parsed = IngestionJobStatus::parse(s);
            assert_eq!(parsed, Some(status));
        }
    }

    #[test]
    fn test_context_type_roundtrip() {
        for ctx in [
            IngestionContextType::Spontaneous,
            IngestionContextType::DownloadRequest,
        ] {
            let s = ctx.as_str();
            let parsed = IngestionContextType::parse(s);
            assert_eq!(parsed, Some(ctx));
        }
    }

    #[test]
    fn test_job_creation() {
        let job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 12)
            .with_context(IngestionContextType::Spontaneous, None);

        assert_eq!(job.id, "job1");
        assert_eq!(job.status, IngestionJobStatus::Pending);
        assert_eq!(job.file_count, 12);
        assert_eq!(job.context_type, Some(IngestionContextType::Spontaneous));
    }

    #[test]
    fn test_file_creation() {
        let file = IngestionFile::new(
            "file1",
            "job1",
            "01 - Track One.mp3",
            5000000,
            "/tmp/job1/01 - Track One.mp3",
        );

        assert_eq!(file.id, "file1");
        assert_eq!(file.job_id, "job1");
        assert!(!file.converted);
    }
}
