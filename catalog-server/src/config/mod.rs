mod file_config;

pub use file_config::{BackgroundJobsConfig, DownloadManagerConfig, FileConfig};

use crate::server::RequestsLoggingLevel;
use clap::ValueEnum;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    // Core settings
    pub db_dir: PathBuf,
    pub media_path: PathBuf,
    pub port: u16,
    pub metrics_port: u16,
    pub logging_level: RequestsLoggingLevel,
    pub content_cache_age_sec: usize,
    pub frontend_dir_path: Option<String>,
    pub downloader_url: Option<String>,
    pub downloader_timeout_sec: u64,
    pub event_retention_days: u64,
    pub prune_interval_hours: u64,

    // Feature configs (with defaults)
    pub download_manager: DownloadManagerSettings,
    pub background_jobs: BackgroundJobsSettings,
}

impl AppConfig {
    pub fn catalog_db_path(&self) -> PathBuf {
        self.db_dir.join("catalog.db")
    }

    pub fn user_db_path(&self) -> PathBuf {
        self.db_dir.join("user.db")
    }

    pub fn server_db_path(&self) -> PathBuf {
        self.db_dir.join("server.db")
    }

    pub fn download_queue_db_path(&self) -> PathBuf {
        self.db_dir.join("download_queue.db")
    }
}

#[derive(Debug, Clone)]
pub struct DownloadManagerSettings {
    pub enabled: bool, // true if downloader_url is set
    pub max_albums_per_hour: u32,
    pub max_albums_per_day: u32,
    pub user_max_requests_per_day: u32,
    pub user_max_queue_size: u32,
    pub process_interval_secs: u64,
    pub stale_in_progress_threshold_secs: u64,
    pub max_retries: u32,
    pub initial_backoff_secs: u64,
    pub max_backoff_secs: u64,
    pub backoff_multiplier: f64,
    pub audit_log_retention_days: u64,
}

impl Default for DownloadManagerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_albums_per_hour: 10,
            max_albums_per_day: 60,
            user_max_requests_per_day: 100,
            user_max_queue_size: 200,
            process_interval_secs: 5,
            stale_in_progress_threshold_secs: 3600,
            max_retries: 5,
            initial_backoff_secs: 60,
            max_backoff_secs: 3600,
            backoff_multiplier: 2.0,
            audit_log_retention_days: 90,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundJobsSettings {
    // Future: per-job settings can be added here
}

/// Parses a logging level string into RequestsLoggingLevel.
/// Uses clap's ValueEnum trait for parsing.
fn parse_logging_level(s: &str) -> Option<RequestsLoggingLevel> {
    RequestsLoggingLevel::from_str(s, true).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_logging_level() {
        assert!(matches!(
            parse_logging_level("none"),
            Some(RequestsLoggingLevel::None)
        ));
        assert!(matches!(
            parse_logging_level("path"),
            Some(RequestsLoggingLevel::Path)
        ));
        assert!(matches!(
            parse_logging_level("headers"),
            Some(RequestsLoggingLevel::Headers)
        ));
        assert!(matches!(
            parse_logging_level("body"),
            Some(RequestsLoggingLevel::Body)
        ));
        // Case insensitive
        assert!(matches!(
            parse_logging_level("PATH"),
            Some(RequestsLoggingLevel::Path)
        ));
        // Invalid
        assert!(parse_logging_level("invalid").is_none());
    }
}
