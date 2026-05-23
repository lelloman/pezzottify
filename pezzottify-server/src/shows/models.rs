use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowStatus {
    Draft,
    ScriptReady,
    Synthesizing,
    Ready,
    Published,
    Failed,
}

impl ShowStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ShowStatus::Draft => "draft",
            ShowStatus::ScriptReady => "script_ready",
            ShowStatus::Synthesizing => "synthesizing",
            ShowStatus::Ready => "ready",
            ShowStatus::Published => "published",
            ShowStatus::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "script_ready" => Some(Self::ScriptReady),
            "synthesizing" => Some(Self::Synthesizing),
            "ready" => Some(Self::Ready),
            "published" => Some(Self::Published),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowSegmentKind {
    Track,
    Narration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSpeaker {
    pub id: String,
    pub name: String,
    pub voice_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSource {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSegment {
    pub id: String,
    pub kind: ShowSegmentKind,
    pub title: String,
    pub track_id: Option<String>,
    pub speaker_id: Option<String>,
    pub text: Option<String>,
    pub audio_path: Option<String>,
    pub mime_type: Option<String>,
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub id: String,
    pub title: String,
    pub status: ShowStatus,
    pub brief: String,
    pub summary: String,
    pub language: String,
    pub target_duration_minutes: i32,
    pub created_by_user_id: usize,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: Option<i64>,
    #[serde(default)]
    pub speakers: Vec<ShowSpeaker>,
    #[serde(default)]
    pub segments: Vec<ShowSegment>,
    #[serde(default)]
    pub sources: Vec<ShowSource>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSummary {
    pub id: String,
    pub title: String,
    pub status: ShowStatus,
    pub summary: String,
    pub language: String,
    pub target_duration_minutes: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: Option<i64>,
    pub segment_count: usize,
    pub track_count: usize,
}

impl From<&Show> for ShowSummary {
    fn from(show: &Show) -> Self {
        Self {
            id: show.id.clone(),
            title: show.title.clone(),
            status: show.status,
            summary: show.summary.clone(),
            language: show.language.clone(),
            target_duration_minutes: show.target_duration_minutes,
            created_at: show.created_at,
            updated_at: show.updated_at,
            published_at: show.published_at,
            segment_count: show.segments.len(),
            track_count: show
                .segments
                .iter()
                .filter(|s| s.kind == ShowSegmentKind::Track)
                .count(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateShowDraftRequest {
    pub brief: String,
    pub target_duration_minutes: Option<i32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateShowScriptRequest {
    pub title: String,
    pub summary: String,
    pub language: String,
    pub target_duration_minutes: i32,
    #[serde(default)]
    pub speakers: Vec<ShowSpeaker>,
    #[serde(default)]
    pub segments: Vec<ShowSegment>,
    #[serde(default)]
    pub sources: Vec<ShowSource>,
}
