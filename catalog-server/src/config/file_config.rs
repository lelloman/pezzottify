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
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DownloadManagerConfig {
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
    /// Search engine to use: "pezzothash", "fts5", "noop"
    pub engine: Option<String>,
}

impl FileConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse config file: {:?}", path))
    }
}
