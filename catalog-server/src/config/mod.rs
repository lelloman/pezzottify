mod file_config;

pub use file_config::{BackgroundJobsConfig, DownloadManagerConfig, FileConfig, SslConfig};

use crate::server::RequestsLoggingLevel;
use anyhow::{bail, Result};
use clap::ValueEnum;
use std::path::PathBuf;

/// CLI arguments that can be used for config resolution.
/// This struct mirrors the CLI arguments that can be overridden by TOML config.
#[derive(Debug, Clone, Default)]
pub struct CliConfig {
    pub db_dir: Option<PathBuf>,
    pub media_path: Option<PathBuf>,
    pub port: u16,
    pub metrics_port: u16,
    pub logging_level: RequestsLoggingLevel,
    pub content_cache_age_sec: usize,
    pub frontend_dir_path: Option<String>,
    pub downloader_url: Option<String>,
    pub downloader_timeout_sec: u64,
    pub event_retention_days: u64,
    pub prune_interval_hours: u64,
    pub ssl_cert: Option<PathBuf>,
    pub ssl_key: Option<PathBuf>,
}

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

    // SSL/TLS configuration
    pub ssl: Option<SslSettings>,
}

#[derive(Debug, Clone)]
pub struct SslSettings {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl AppConfig {
    /// Resolve configuration from CLI arguments and optional TOML file config.
    /// TOML values override CLI values where present.
    pub fn resolve(cli: &CliConfig, file_config: Option<FileConfig>) -> Result<Self> {
        let file = file_config.unwrap_or_default();

        // TOML overrides CLI for each field
        let db_dir = file
            .db_dir
            .map(PathBuf::from)
            .or_else(|| cli.db_dir.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("db_dir must be specified via --db-dir or in config file")
            })?;

        // Validate db_dir exists
        if !db_dir.exists() {
            bail!("Database directory does not exist: {:?}", db_dir);
        }
        if !db_dir.is_dir() {
            bail!("db_dir is not a directory: {:?}", db_dir);
        }

        let media_path = file
            .media_path
            .map(PathBuf::from)
            .or_else(|| cli.media_path.clone())
            .unwrap_or_else(|| db_dir.clone());

        let port = file.port.unwrap_or(cli.port);
        let metrics_port = file.metrics_port.unwrap_or(cli.metrics_port);

        let logging_level = file
            .logging_level
            .and_then(|s| parse_logging_level(&s))
            .unwrap_or_else(|| cli.logging_level.clone());

        let content_cache_age_sec = file
            .content_cache_age_sec
            .unwrap_or(cli.content_cache_age_sec);
        let frontend_dir_path = file
            .frontend_dir_path
            .or_else(|| cli.frontend_dir_path.clone());

        let downloader_url = file
            .downloader_url
            .clone()
            .or_else(|| cli.downloader_url.clone());

        let downloader_timeout_sec = file
            .downloader_timeout_sec
            .unwrap_or(cli.downloader_timeout_sec);
        let event_retention_days = file
            .event_retention_days
            .unwrap_or(cli.event_retention_days);
        let prune_interval_hours = file
            .prune_interval_hours
            .unwrap_or(cli.prune_interval_hours);

        // Download manager settings - merge file config with defaults
        let dm_file = file.download_manager.unwrap_or_default();
        let download_manager = DownloadManagerSettings {
            enabled: downloader_url.is_some(),
            max_albums_per_hour: dm_file.max_albums_per_hour.unwrap_or(10),
            max_albums_per_day: dm_file.max_albums_per_day.unwrap_or(60),
            user_max_requests_per_day: dm_file.user_max_requests_per_day.unwrap_or(100),
            user_max_queue_size: dm_file.user_max_queue_size.unwrap_or(200),
            process_interval_secs: dm_file.process_interval_secs.unwrap_or(5),
            stale_in_progress_threshold_secs: dm_file
                .stale_in_progress_threshold_secs
                .unwrap_or(3600),
            max_retries: dm_file.max_retries.unwrap_or(8),
            initial_backoff_secs: dm_file.initial_backoff_secs.unwrap_or(60),
            max_backoff_secs: dm_file.max_backoff_secs.unwrap_or(86400), // 24 hours
            backoff_multiplier: dm_file.backoff_multiplier.unwrap_or(2.5),
            audit_log_retention_days: dm_file.audit_log_retention_days.unwrap_or(90),
            // Throttle settings
            throttle_enabled: dm_file.throttle_enabled.unwrap_or(true),
            throttle_max_mb_per_minute: dm_file.throttle_max_mb_per_minute.unwrap_or(20),
            throttle_max_mb_per_hour: dm_file.throttle_max_mb_per_hour.unwrap_or(1500),
        };

        let background_jobs = BackgroundJobsSettings::default();

        // SSL settings - TOML [ssl] section takes precedence over CLI args
        let ssl = if let Some(ssl_file) = file.ssl {
            // Validate paths exist
            let cert_path = PathBuf::from(&ssl_file.cert_path);
            let key_path = PathBuf::from(&ssl_file.key_path);
            if !cert_path.exists() {
                bail!("SSL certificate file not found: {:?}", cert_path);
            }
            if !key_path.exists() {
                bail!("SSL key file not found: {:?}", key_path);
            }
            Some(SslSettings {
                cert_path,
                key_path,
            })
        } else if let (Some(cert), Some(key)) = (&cli.ssl_cert, &cli.ssl_key) {
            // CLI args provided
            if !cert.exists() {
                bail!("SSL certificate file not found: {:?}", cert);
            }
            if !key.exists() {
                bail!("SSL key file not found: {:?}", key);
            }
            Some(SslSettings {
                cert_path: cert.clone(),
                key_path: key.clone(),
            })
        } else if cli.ssl_cert.is_some() || cli.ssl_key.is_some() {
            // Only one of cert/key provided - error
            bail!("Both --ssl-cert and --ssl-key must be provided together");
        } else {
            None
        };

        Ok(Self {
            db_dir,
            media_path,
            port,
            metrics_port,
            logging_level,
            content_cache_age_sec,
            frontend_dir_path,
            downloader_url,
            downloader_timeout_sec,
            event_retention_days,
            prune_interval_hours,
            download_manager,
            background_jobs,
            ssl,
        })
    }

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
    // Throttle settings
    pub throttle_enabled: bool,
    pub throttle_max_mb_per_minute: u64,
    pub throttle_max_mb_per_hour: u64,
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
            max_retries: 8,
            initial_backoff_secs: 60,
            max_backoff_secs: 86400, // 24 hours
            backoff_multiplier: 2.5,
            audit_log_retention_days: 90,
            // Throttle defaults
            throttle_enabled: true,
            throttle_max_mb_per_minute: 20,
            throttle_max_mb_per_hour: 1500,
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
    use tempfile::TempDir;

    fn make_temp_db_dir() -> TempDir {
        TempDir::new().unwrap()
    }

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

    #[test]
    fn test_resolve_cli_only() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            media_path: Some(PathBuf::from("/media")),
            port: 3001,
            metrics_port: 9091,
            logging_level: RequestsLoggingLevel::Headers,
            content_cache_age_sec: 7200,
            frontend_dir_path: Some("/frontend".to_string()),
            downloader_url: Some("http://downloader:3002".to_string()),
            downloader_timeout_sec: 600,
            event_retention_days: 60,
            prune_interval_hours: 12,
            ssl_cert: None,
            ssl_key: None,
        };

        let config = AppConfig::resolve(&cli, None).unwrap();

        assert_eq!(config.db_dir, temp_dir.path());
        assert_eq!(config.media_path, PathBuf::from("/media"));
        assert_eq!(config.port, 3001);
        assert_eq!(config.metrics_port, 9091);
        assert_eq!(config.logging_level, RequestsLoggingLevel::Headers);
        assert_eq!(config.content_cache_age_sec, 7200);
        assert_eq!(config.frontend_dir_path, Some("/frontend".to_string()));
        assert_eq!(
            config.downloader_url,
            Some("http://downloader:3002".to_string())
        );
        assert_eq!(config.downloader_timeout_sec, 600);
        assert_eq!(config.event_retention_days, 60);
        assert_eq!(config.prune_interval_hours, 12);
        assert!(config.download_manager.enabled);
        assert!(config.ssl.is_none());
    }

    #[test]
    fn test_resolve_toml_overrides_cli() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(PathBuf::from("/should/be/overridden")),
            media_path: Some(PathBuf::from("/cli/media")),
            port: 3001,
            metrics_port: 9091,
            logging_level: RequestsLoggingLevel::Path,
            content_cache_age_sec: 3600,
            ..Default::default()
        };

        let file_config = FileConfig {
            db_dir: Some(temp_dir.path().to_string_lossy().to_string()),
            media_path: Some("/toml/media".to_string()),
            port: Some(4000),
            logging_level: Some("body".to_string()),
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, Some(file_config)).unwrap();

        // TOML values should override CLI
        assert_eq!(config.db_dir, temp_dir.path());
        assert_eq!(config.media_path, PathBuf::from("/toml/media"));
        assert_eq!(config.port, 4000);
        assert_eq!(config.logging_level, RequestsLoggingLevel::Body);
        // CLI value used when TOML doesn't specify
        assert_eq!(config.metrics_port, 9091);
        assert_eq!(config.content_cache_age_sec, 3600);
    }

    #[test]
    fn test_resolve_missing_db_dir_error() {
        let cli = CliConfig::default();
        let result = AppConfig::resolve(&cli, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("db_dir must be specified"));
    }

    #[test]
    fn test_resolve_nonexistent_db_dir_error() {
        let cli = CliConfig {
            db_dir: Some(PathBuf::from("/nonexistent/path/that/should/not/exist")),
            ..Default::default()
        };
        let result = AppConfig::resolve(&cli, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_resolve_db_dir_not_directory_error() {
        // Create a temporary file (not a directory)
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let cli = CliConfig {
            db_dir: Some(temp_file.path().to_path_buf()),
            ..Default::default()
        };
        let result = AppConfig::resolve(&cli, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn test_resolve_download_manager_disabled_without_url() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            downloader_url: None,
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, None).unwrap();
        assert!(!config.download_manager.enabled);
    }

    #[test]
    fn test_resolve_download_manager_enabled_with_url() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            downloader_url: Some("http://localhost:3002".to_string()),
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, None).unwrap();
        assert!(config.download_manager.enabled);
    }

    #[test]
    fn test_resolve_media_path_defaults_to_db_dir() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            media_path: None,
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, None).unwrap();
        assert_eq!(config.media_path, temp_dir.path());
    }

    #[test]
    fn test_db_path_helpers() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, None).unwrap();

        assert_eq!(config.catalog_db_path(), temp_dir.path().join("catalog.db"));
        assert_eq!(config.user_db_path(), temp_dir.path().join("user.db"));
        assert_eq!(config.server_db_path(), temp_dir.path().join("server.db"));
        assert_eq!(
            config.download_queue_db_path(),
            temp_dir.path().join("download_queue.db")
        );
    }
}
