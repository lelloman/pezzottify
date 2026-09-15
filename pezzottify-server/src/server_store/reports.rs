//! Versioned report API types. Content size limits are UTF-8 bytes.
use serde::{Deserialize, Serialize};

pub const MAX_ASSISTANT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_LOG_BYTES: usize = 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 20 * 1024 * 1024;
pub type ReportResult<T> = Result<T, ReportError>;

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("Invalid report: {0}")]
    Invalid(String),
    #[error("Report payload exceeds the supported limit")]
    TooLarge,
    #[error("Report not found")]
    NotFound,
    #[error("Report changed, or the request key was reused with different content")]
    Conflict,
    #[error("Report quota exceeded")]
    Quota,
    #[error("Report storage is at capacity")]
    Capacity,
    #[error("Report storage failed")]
    Storage,
}
impl From<rusqlite::Error> for ReportError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl From<serde_json::Error> for ReportError {
    fn from(_: serde_json::Error) -> Self {
        Self::Invalid("Invalid JSON".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewAttachment {
    pub kind: String,
    pub content: String,
    pub consent: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewReport {
    pub client_request_id: String,
    pub kind: String,
    pub category: String,
    pub title: Option<String>,
    pub description: String,
    pub client_type: String,
    pub client_version: Option<String>,
    pub device_info: Option<String>,
    #[serde(default)]
    pub attachments: Vec<NewAttachment>,
}
impl NewReport {
    pub fn validate(&self) -> ReportResult<()> {
        if uuid::Uuid::parse_str(&self.client_request_id).is_err()
            || !["bug", "feature"].contains(&self.kind.as_str())
            || !["assistant", "playback", "downloads", "ui", "other"]
                .contains(&self.category.as_str())
            || !["web", "android"].contains(&self.client_type.as_str())
            || self.description.trim().is_empty()
        {
            return Err(ReportError::Invalid(
                "Invalid kind/category/client/request ID or empty description".into(),
            ));
        }
        if self.title.as_ref().map_or(0, String::len) > 200
            || self.description.len() > 100 * 1024
            || self.client_version.as_ref().map_or(0, String::len)
                + self.device_info.as_ref().map_or(0, String::len)
                > 4096
            || self.attachments.len() > 2
        {
            return Err(ReportError::TooLarge);
        }
        let mut kinds = std::collections::HashSet::new();
        for a in &self.attachments {
            if !a.consent || !kinds.insert(&a.kind) {
                return Err(ReportError::Invalid(
                    "Attachment consent required; duplicate kinds forbidden".into(),
                ));
            }
            let cap = match a.kind.as_str() {
                "technical_logs" => MAX_LOG_BYTES,
                "assistant" => MAX_ASSISTANT_BYTES,
                _ => return Err(ReportError::Invalid("Unsupported attachment kind".into())),
            };
            if a.content.len() > cap {
                return Err(ReportError::TooLarge);
            }
            if a.kind == "assistant" {
                validate_diagnostics(&a.content)?;
            }
        }
        Ok(())
    }
}

pub fn validate_diagnostics(content: &str) -> ReportResult<()> {
    let envelope: serde_json::Value = serde_json::from_str(content)?;
    if envelope.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
        || envelope
            .get("captured_at")
            .and_then(|v| v.as_str())
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
            .is_none()
    {
        return Err(ReportError::Invalid(
            "Unsupported diagnostic schema or timestamp".into(),
        ));
    }
    let turns = envelope
        .get("turns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ReportError::Invalid("Missing turns".into()))?;
    if turns.len() > 4096 {
        return Err(ReportError::TooLarge);
    }
    for turn in turns {
        if !["running", "completed", "failed", "cancelled", "interrupted"]
            .contains(&turn["status"].as_str().unwrap_or(""))
            || turn["id"].as_str().is_none()
            || turn["conversation_id"].as_str().is_none()
            || turn["started_at"]
                .as_str()
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                .is_none()
        {
            return Err(ReportError::Invalid("Invalid diagnostic turn".into()));
        }
        let events = turn["events"]
            .as_array()
            .ok_or_else(|| ReportError::Invalid("Missing events".into()))?;
        if events.len() > 8192 {
            return Err(ReportError::TooLarge);
        }
        for event in events {
            if ![
                "user",
                "assistant",
                "tool_call",
                "tool_result",
                "confirmation",
                "error",
                "cancellation",
                "compaction",
                "metadata",
            ]
            .contains(&event["kind"].as_str().unwrap_or(""))
                || event["timestamp"]
                    .as_str()
                    .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                    .is_none()
            {
                return Err(ReportError::Invalid("Invalid diagnostic event".into()));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: String,
    pub kind: String,
    pub size_bytes: usize,
    pub created_at: i64,
    pub expires_at: i64,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub seq: i64,
    pub id: String,
    pub user_id: usize,
    pub user_handle: String,
    pub kind: String,
    pub category: String,
    pub status: String,
    pub version: i64,
    pub title: Option<String>,
    pub description: String,
    pub client_type: String,
    pub client_version: Option<String>,
    pub device_info: Option<String>,
    pub created_at: String,
    pub updated_at: i64,
    pub duplicate_of: Option<String>,
    pub attachments: Vec<AttachmentInfo>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPage {
    pub items: Vec<Report>,
    pub next_cursor: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportFilter {
    pub before: Option<i64>,
    pub limit: Option<usize>,
    pub kind: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub client_type: Option<String>,
    pub client_version: Option<String>,
    pub user_id: Option<usize>,
    pub has_diagnostics: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportUpdate {
    pub expected_version: i64,
    pub status: Option<String>,
    pub note: Option<String>,
    /// Empty string clears an existing duplicate link.
    pub duplicate_of: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReportEvent {
    pub seq: i64,
    pub id: String,
    pub actor_id: usize,
    pub kind: String,
    pub data: serde_json::Value,
    pub created_at: i64,
}
#[derive(Debug, Serialize)]
pub struct EventPage {
    pub items: Vec<ReportEvent>,
    pub next_cursor: Option<i64>,
}
#[derive(Debug, Serialize)]
pub struct ReportReceipt {
    pub id: String,
    pub version: i64,
    pub replayed: bool,
}
#[derive(Debug, Serialize)]
pub struct AttachmentContent {
    pub attachment: AttachmentInfo,
    pub content: String,
}

pub trait ReportRepository: Send + Sync {
    fn create(
        &self,
        user_id: usize,
        user_handle: &str,
        input: NewReport,
    ) -> ReportResult<ReportReceipt>;
    fn list(&self, owner: Option<usize>, filter: ReportFilter) -> ReportResult<ReportPage>;
    fn get(&self, id: &str, owner: Option<usize>) -> ReportResult<Report>;
    fn update(&self, id: &str, actor: usize, input: ReportUpdate) -> ReportResult<Report>;
    fn events(&self, id: &str, before: Option<i64>, limit: usize) -> ReportResult<EventPage>;
    fn attachment(
        &self,
        id: &str,
        attachment: &str,
        owner: Option<usize>,
        actor: usize,
    ) -> ReportResult<AttachmentContent>;
    fn delete_attachment(
        &self,
        id: &str,
        attachment: &str,
        owner: Option<usize>,
        actor: usize,
    ) -> ReportResult<()>;
}
