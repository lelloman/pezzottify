use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FileConfig {
    // Core settings (can override CLI)
    pub db_dir: Option<String>,
    pub media_path: Option<String>,
    pub port: Option<u16>,
    pub metrics_port: Option<u16>,
    pub logging_level: Option<String>,
    pub content_cache_age_sec: Option<usize>,
    pub frontend_dir_path: Option<String>,
    pub downloader_url: Option<String>,
    pub downloader_timeout_sec: Option<u64>,
    pub event_retention_days: Option<u64>,
    pub prune_interval_hours: Option<u64>,

    // Feature configs
    pub download_manager: Option<DownloadManagerConfig>,
    pub background_jobs: Option<BackgroundJobsConfig>,
    pub search: Option<SearchConfig>,
    pub oidc: Option<OidcConfig>,
    pub catalog_store: Option<CatalogStoreConfig>,
    pub agent: Option<AgentConfig>,
    pub ingestion: Option<IngestionConfig>,
    pub related_artists: Option<RelatedArtistsConfig>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CatalogStoreConfig {
    pub read_pool_size: Option<usize>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DownloadManagerConfig {
    /// Enable the download manager queue (allows users to request downloads).
    /// Defaults to true. Set to false to disable the feature entirely.
    pub enabled: Option<bool>,
    pub max_albums_per_hour: Option<u32>,
    pub max_albums_per_day: Option<u32>,
    pub user_max_requests_per_day: Option<u32>,
    pub user_max_queue_size: Option<u32>,
    pub stale_in_progress_threshold_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub initial_backoff_secs: Option<u64>,
    pub max_backoff_secs: Option<u64>,
    pub backoff_multiplier: Option<f64>,
    pub audit_log_retention_days: Option<u64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct BackgroundJobsConfig {
    pub popular_content: Option<PopularContentJobConfig>,
    pub whatsnew_batch: Option<IntervalJobConfig>,
    pub ingestion_cleanup: Option<IngestionCleanupJobConfig>,
    pub audit_log_cleanup: Option<AuditLogCleanupJobConfig>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PopularContentJobConfig {
    pub interval_hours: Option<u64>,
    pub albums_limit: Option<usize>,
    pub artists_limit: Option<usize>,
    pub lookback_days: Option<u32>,
    pub impression_lookback_days: Option<u32>,
    pub impression_retention_days: Option<u32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct IntervalJobConfig {
    pub interval_hours: Option<u64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct IngestionCleanupJobConfig {
    pub interval_hours: Option<u64>,
    pub min_age_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct AuditLogCleanupJobConfig {
    pub interval_hours: Option<u64>,
    pub retention_days: Option<u64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct SearchConfig {
    /// Search engine: "fts5-levenshtein" (default), "noop" (disabled)
    pub engine: Option<String>,
    /// Streaming search configuration
    pub streaming: Option<StreamingSearchConfig>,
}

/// Configuration for streaming structured search
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct StreamingSearchConfig {
    /// Target identification strategy: "score_gap" (default)
    pub strategy: Option<String>,

    // ScoreGap strategy settings
    /// Minimum normalized score for top result (0.0 - 1.0)
    pub min_absolute_score: Option<f64>,
    /// Minimum gap between #1 and #2 as ratio of #1's score
    pub min_score_gap_ratio: Option<f64>,
    /// Additional confidence boost for exact name matches
    pub exact_match_boost: Option<f64>,

    // Enrichment limits
    /// Maximum number of popular tracks to include
    pub popular_tracks_limit: Option<usize>,
    /// Maximum number of albums to include
    pub albums_limit: Option<usize>,
    /// Maximum number of related artists to include
    pub related_artists_limit: Option<usize>,
    /// Maximum number of other results to include
    pub other_results_limit: Option<usize>,
    /// Maximum number of top results when no target is identified
    pub top_results_limit: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OidcConfig {
    /// OIDC provider URL (issuer), e.g., "https://auth.lelloman.com"
    pub provider_url: String,
    /// OAuth2 client ID
    pub client_id: String,
    /// OAuth2 client secret
    pub client_secret: String,
    /// Redirect URI for the callback, e.g., "https://pezzottify.lelloman.com/v1/auth/callback"
    pub redirect_uri: String,
    /// OAuth2 scopes to request (defaults to ["openid", "profile", "email"])
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    /// If true, disables the password authentication endpoint when OIDC is configured.
    /// Users must authenticate via OIDC only. Default: false
    #[serde(default)]
    pub disable_password_auth: bool,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

/// Configuration for the agent LLM backend.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct AgentConfig {
    /// Whether agent features are enabled.
    pub enabled: Option<bool>,
    /// Maximum iterations for agent workflows.
    pub max_iterations: Option<usize>,
    /// LLM configuration.
    pub llm: Option<AgentLlmConfig>,
}

/// Configuration for the LLM provider used by agents.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct AgentLlmConfig {
    /// LLM provider: "ollama" (default), "openai" for OpenAI-compatible APIs.
    pub provider: Option<String>,
    /// Base URL for the LLM API (e.g., "http://localhost:11434" for Ollama,
    /// "https://api.openai.com/v1" for OpenAI).
    pub base_url: Option<String>,
    /// Model to use (e.g., "llama3.1:8b" for Ollama, "gpt-4o-mini" for OpenAI).
    pub model: Option<String>,
    /// Static API key for authentication. Mutually exclusive with api_key_command.
    pub api_key: Option<String>,
    /// Shell command to fetch API key dynamically (executed before each request).
    /// Use this for rotating tokens. Example: "cat /run/secrets/openai-key"
    /// Mutually exclusive with api_key.
    pub api_key_command: Option<String>,
    /// Temperature for sampling (0.0 = deterministic, 1.0 = creative).
    pub temperature: Option<f32>,
    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,
}

/// Configuration for the ingestion feature.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct IngestionConfig {
    /// Whether ingestion is enabled (requires agent to be enabled).
    pub enabled: Option<bool>,
    /// Temporary directory for uploaded files.
    pub temp_dir: Option<String>,
    /// Maximum upload size in megabytes.
    pub max_upload_size_mb: Option<u64>,
    /// Confidence threshold for auto-approval (0.0 - 1.0).
    pub auto_approve_threshold: Option<f32>,
    /// Path to ffmpeg binary.
    pub ffmpeg_path: Option<String>,
    /// Path to ffprobe binary.
    pub ffprobe_path: Option<String>,
    /// Output bitrate for converted audio (e.g., "320k").
    pub output_bitrate: Option<String>,
}

/// Configuration for related artists enrichment via MusicBrainz + Last.fm.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct RelatedArtistsConfig {
    /// Whether the related artists enrichment job is enabled.
    pub enabled: Option<bool>,
    /// Last.fm API key (required when enabled).
    pub lastfm_api_key: Option<String>,
    /// MusicBrainz User-Agent string (required when enabled).
    /// Should follow format: "AppName/Version (contact@example.com)"
    pub musicbrainz_user_agent: Option<String>,
    /// Number of artists to process per phase per job run.
    pub batch_size: Option<usize>,
    /// Maximum number of similar artists to fetch from Last.fm per artist.
    pub similar_artists_limit: Option<usize>,
    /// Interval in hours between job runs.
    pub interval_hours: Option<u64>,
}

impl FileConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse config file: {:?}", path))
    }
}
