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
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CatalogStoreConfig {
    pub read_pool_size: Option<usize>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DownloadManagerConfig {
    /// Quentin Torrentino HTTP base URL (e.g., "http://localhost:8080")
    pub qt_base_url: Option<String>,
    /// Quentin Torrentino WebSocket URL (e.g., "ws://localhost:8080/ws")
    pub qt_ws_url: Option<String>,
    /// Quentin Torrentino auth token
    pub qt_auth_token: Option<String>,
    pub max_albums_per_hour: Option<u32>,
    pub max_albums_per_day: Option<u32>,
    pub user_max_requests_per_day: Option<u32>,
    pub user_max_queue_size: Option<u32>,
    pub process_interval_secs: Option<u64>,
    pub stale_in_progress_threshold_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub initial_backoff_secs: Option<u64>,
    pub max_backoff_secs: Option<u64>,
    pub backoff_multiplier: Option<f64>,
    pub audit_log_retention_days: Option<u64>,
    // Throttle settings
    pub throttle_enabled: Option<bool>,
    pub throttle_max_mb_per_minute: Option<u64>,
    pub throttle_max_mb_per_hour: Option<u64>,
    // Corruption handler settings
    pub corruption_window_size: Option<usize>,
    pub corruption_failure_threshold: Option<usize>,
    pub corruption_base_cooldown_secs: Option<u64>,
    pub corruption_max_cooldown_secs: Option<u64>,
    pub corruption_cooldown_multiplier: Option<f64>,
    pub corruption_successes_to_deescalate: Option<u32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct BackgroundJobsConfig {
    // Future: per-job configuration can be added here
    // e.g., pub popular_content_interval_hours: Option<u64>,
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

impl FileConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse config file: {:?}", path))
    }
}
