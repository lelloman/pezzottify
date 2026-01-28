//! WebSocket message types.
//!
//! Defines the generic message envelope format used for all WebSocket communication.
//! Feature-specific payloads are carried as JSON values, allowing extensibility.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Server -> Client message envelope.
///
/// All messages from server to client use this format. The `msg_type` field
/// is used for routing (e.g., "connected", "sync.liked_changed", "playback.pause").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMessage {
    /// Message type identifier (e.g., "sync", "playback", "notification")
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Feature-specific payload (JSON value)
    pub payload: serde_json::Value,
}

impl ServerMessage {
    /// Create a new server message with the given type and payload.
    pub fn new(msg_type: impl Into<String>, payload: impl Serialize) -> Self {
        Self {
            msg_type: msg_type.into(),
            payload: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
        }
    }

    /// Create a server message with a null payload.
    pub fn empty(msg_type: impl Into<String>) -> Self {
        Self {
            msg_type: msg_type.into(),
            payload: serde_json::Value::Null,
        }
    }
}

/// Client -> Server message envelope.
///
/// All messages from client to server use this format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientMessage {
    /// Message type identifier
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Feature-specific payload (JSON value)
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// System-level messages (not feature-specific).
///
/// These are reserved message types used by the WebSocket infrastructure itself.
pub mod system {
    use serde::{Deserialize, Serialize};

    /// Sent immediately after connection is established.
    ///
    /// Confirms the WebSocket connection is ready and provides the device ID and server version.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Connected {
        pub device_id: usize,
        pub server_version: String,
    }

    /// Heartbeat request (client -> server).
    ///
    /// Client can send this to check connection is alive.
    /// Server responds with `Pong`.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Ping;

    /// Heartbeat response (server -> client).
    ///
    /// Sent in response to client `Ping`.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Pong;

    /// Error message (server -> client).
    ///
    /// Sent when the server encounters an error processing a client message.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Error {
        pub code: String,
        pub message: String,
    }

    impl Error {
        pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
            Self {
                code: code.into(),
                message: message.into(),
            }
        }
    }
}

/// Reserved message type constants.
pub mod msg_types {
    /// Sent by server on successful connection.
    pub const CONNECTED: &str = "connected";
    /// Client heartbeat request.
    pub const PING: &str = "ping";
    /// Server heartbeat response.
    pub const PONG: &str = "pong";
    /// Server error response.
    pub const ERROR: &str = "error";
    /// Sync event notification (server -> client).
    pub const SYNC: &str = "sync";
    /// Catalog updated notification (server -> all clients).
    pub const CATALOG_UPDATED: &str = "catalog_updated";
    /// Ingestion job update notification (server -> client).
    pub const INGESTION_UPDATE: &str = "ingestion_update";
    /// Ingestion job progress update (server -> client).
    pub const INGESTION_PROGRESS: &str = "ingestion_progress";
    /// Ingestion match found (server -> client).
    pub const INGESTION_MATCH_FOUND: &str = "ingestion_match_found";
    /// Ingestion review needed (server -> client).
    pub const INGESTION_REVIEW_NEEDED: &str = "ingestion_review_needed";
    /// Ingestion job completed (server -> client).
    pub const INGESTION_COMPLETED: &str = "ingestion_completed";
    /// Ingestion job failed (server -> client).
    pub const INGESTION_FAILED: &str = "ingestion_failed";
}

/// Sync-related message payloads.
pub mod sync {
    use crate::user::sync_events::StoredEvent;
    use serde::{Deserialize, Serialize};

    /// Payload for sync event messages.
    ///
    /// Sent to notify clients of state changes (likes, settings, playlists, permissions).
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct SyncEventMessage {
        pub event: StoredEvent,
    }
}

/// Catalog-related message payloads.
pub mod catalog {
    use serde::{Deserialize, Serialize};

    /// Payload for catalog_updated messages.
    ///
    /// Broadcast to ALL connected clients when new content is added to the catalog.
    /// Clients should re-sync their skeleton data upon receiving this.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct CatalogUpdatedMessage {
        /// Current skeleton version on server.
        pub skeleton_version: i64,
    }
}

/// Ingestion-related message payloads.
pub mod ingestion {
    use serde::{Deserialize, Serialize};

    /// Payload for ingestion_update messages.
    ///
    /// Sent to the user who initiated the ingestion when job state changes.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IngestionUpdateMessage {
        /// Job ID.
        pub job_id: String,
        /// New status.
        pub status: String,
        /// Matched track ID (if status is CONVERTING or COMPLETED).
        pub matched_track_id: Option<String>,
        /// Match confidence.
        pub match_confidence: Option<f32>,
        /// Error message (if status is FAILED).
        pub error_message: Option<String>,
        /// True if human review is required.
        pub needs_review: bool,
    }

    impl IngestionUpdateMessage {
        pub fn new(job_id: impl Into<String>, status: impl Into<String>) -> Self {
            Self {
                job_id: job_id.into(),
                status: status.into(),
                matched_track_id: None,
                match_confidence: None,
                error_message: None,
                needs_review: false,
            }
        }

        pub fn with_match(mut self, track_id: impl Into<String>, confidence: f32) -> Self {
            self.matched_track_id = Some(track_id.into());
            self.match_confidence = Some(confidence);
            self
        }

        pub fn with_error(mut self, message: impl Into<String>) -> Self {
            self.error_message = Some(message.into());
            self
        }

        pub fn with_review(mut self) -> Self {
            self.needs_review = true;
            self
        }
    }

    /// Detailed progress update for the ingestion monitor.
    ///
    /// Sent periodically during processing to show real-time progress.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct JobProgressUpdate {
        /// Job ID.
        pub job_id: String,
        /// Current status.
        pub status: String,
        /// Current processing phase.
        pub phase: String,
        /// Progress within current phase (0-100).
        pub phase_progress: u8,
        /// Number of files processed so far.
        pub files_processed: u32,
        /// Total number of files.
        pub files_total: u32,
    }

    impl JobProgressUpdate {
        pub fn new(
            job_id: impl Into<String>,
            status: impl Into<String>,
            phase: impl Into<String>,
        ) -> Self {
            Self {
                job_id: job_id.into(),
                status: status.into(),
                phase: phase.into(),
                phase_progress: 0,
                files_processed: 0,
                files_total: 0,
            }
        }

        pub fn with_progress(mut self, progress: u8, processed: u32, total: u32) -> Self {
            self.phase_progress = progress;
            self.files_processed = processed;
            self.files_total = total;
            self
        }
    }

    /// Album candidate summary for match found messages.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct CandidateSummary {
        pub id: String,
        pub name: String,
        pub artist_name: String,
        pub track_count: i32,
        pub score: f32,
        pub delta_ms: i64,
    }

    /// Sent when an album match is found (either auto or for review).
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct MatchFoundUpdate {
        /// Job ID.
        pub job_id: String,
        /// Matched album ID (if high confidence).
        pub album_id: Option<String>,
        /// Album name.
        pub album_name: Option<String>,
        /// Artist name.
        pub artist_name: Option<String>,
        /// Match confidence (0.0 - 1.0).
        pub confidence: f32,
        /// Ticket type: "SUCCESS", "REVIEW", or "FAILURE".
        pub ticket_type: String,
        /// Top album candidates (for review).
        pub candidates: Vec<CandidateSummary>,
    }

    /// Sent when human review is required.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ReviewNeededUpdate {
        /// Job ID.
        pub job_id: String,
        /// Question to present to user.
        pub question: String,
        /// Available options.
        pub options: Vec<ReviewOptionSummary>,
    }

    /// Review option for WebSocket messages.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ReviewOptionSummary {
        pub id: String,
        pub label: String,
        pub description: Option<String>,
    }

    /// Sent when job completes successfully.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct JobCompletedUpdate {
        /// Job ID.
        pub job_id: String,
        /// Number of tracks added.
        pub tracks_added: u32,
        /// Album name.
        pub album_name: String,
        /// Artist name.
        pub artist_name: String,
    }

    /// Sent when job fails.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct JobFailedUpdate {
        /// Job ID.
        pub job_id: String,
        /// Error message.
        pub error: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_message_serializes_correctly() {
        let msg = ServerMessage::new("test_type", serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"type\":\"test_type\""));
        assert!(json.contains("\"payload\":{\"key\":\"value\"}"));
    }

    #[test]
    fn server_message_deserializes_correctly() {
        let json = r#"{"type":"test_type","payload":{"key":"value"}}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "test_type");
        assert_eq!(msg.payload["key"], "value");
    }

    #[test]
    fn server_message_empty_creates_null_payload() {
        let msg = ServerMessage::empty("ping");
        assert_eq!(msg.msg_type, "ping");
        assert_eq!(msg.payload, serde_json::Value::Null);
    }

    #[test]
    fn client_message_deserializes_correctly() {
        let json = r#"{"type":"ping","payload":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "ping");
    }

    #[test]
    fn client_message_deserializes_without_payload() {
        // Client might omit payload for simple messages like ping
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "ping");
        assert_eq!(msg.payload, serde_json::Value::Null);
    }

    #[test]
    fn system_connected_serializes_correctly() {
        let connected = system::Connected {
            device_id: 42,
            server_version: "1.2.3".to_string(),
        };
        let msg = ServerMessage::new(msg_types::CONNECTED, &connected);
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"type\":\"connected\""));
        assert!(json.contains("\"device_id\":42"));
        assert!(json.contains("\"server_version\":\"1.2.3\""));
    }

    #[test]
    fn system_error_serializes_correctly() {
        let error = system::Error::new("invalid_message", "Could not parse message");
        let msg = ServerMessage::new(msg_types::ERROR, &error);
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"code\":\"invalid_message\""));
        assert!(json.contains("\"message\":\"Could not parse message\""));
    }

    #[test]
    fn system_ping_pong_serialize_correctly() {
        let ping_msg = ServerMessage::new(msg_types::PING, system::Ping);
        let pong_msg = ServerMessage::new(msg_types::PONG, system::Pong);

        // Unit structs serialize as null in serde
        let ping_json = serde_json::to_string(&ping_msg).unwrap();
        let pong_json = serde_json::to_string(&pong_msg).unwrap();

        assert!(ping_json.contains("\"type\":\"ping\""));
        assert!(pong_json.contains("\"type\":\"pong\""));
    }

    #[test]
    fn server_message_with_complex_payload() {
        #[derive(Serialize)]
        struct ComplexPayload {
            items: Vec<String>,
            count: u32,
            nested: NestedData,
        }

        #[derive(Serialize)]
        struct NestedData {
            enabled: bool,
        }

        let payload = ComplexPayload {
            items: vec!["a".to_string(), "b".to_string()],
            count: 2,
            nested: NestedData { enabled: true },
        };

        let msg = ServerMessage::new("complex", payload);
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"items\":[\"a\",\"b\"]"));
        assert!(json.contains("\"count\":2"));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn message_type_constants() {
        assert_eq!(msg_types::CONNECTED, "connected");
        assert_eq!(msg_types::PING, "ping");
        assert_eq!(msg_types::PONG, "pong");
        assert_eq!(msg_types::ERROR, "error");
    }
}
