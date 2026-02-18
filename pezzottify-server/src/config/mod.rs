mod file_config;

pub use file_config::{
    AgentConfig, AgentLlmConfig, AudioAnalysisConfig, AuditLogCleanupJobConfig,
    BackgroundJobsConfig, CatalogAvailabilityStatsJobConfig, CatalogStoreConfig,
    DevicePruningJobConfig, DownloadManagerConfig, FileConfig, IngestionCleanupJobConfig,
    IngestionConfig, IntervalJobConfig, OidcConfig, PopularContentJobConfig,
    RelatedArtistsConfig, SearchConfig, StreamingSearchConfig as StreamingSearchFileConfig,
};

use crate::server::RequestsLoggingLevel;
use anyhow::{bail, Result};
use clap::ValueEnum;
use std::path::PathBuf;

/// Settings for the search subsystem
#[derive(Debug, Clone)]
pub struct SearchSettings {
    /// Search engine to use: "fts5-levenshtein" or "noop"
    pub engine: String,
    pub streaming: StreamingSearchSettings,
}

/// Settings for streaming structured search
#[derive(Debug, Clone)]
pub struct StreamingSearchSettings {
    /// Target identification strategy
    pub strategy: TargetIdentifierStrategy,

    // ScoreGap strategy settings
    /// Minimum normalized score for top result (0.0 - 1.0)
    pub min_absolute_score: f64,
    /// Minimum gap between #1 and #2 as ratio of #1's score
    pub min_score_gap_ratio: f64,
    /// Additional confidence boost for exact name matches
    pub exact_match_boost: f64,

    // Enrichment limits
    /// Maximum number of popular tracks to include
    pub popular_tracks_limit: usize,
    /// Maximum number of albums to include
    pub albums_limit: usize,
    /// Maximum number of related artists to include
    pub related_artists_limit: usize,
    /// Maximum number of other results to include
    pub other_results_limit: usize,
    /// Maximum number of top results when no target is identified
    pub top_results_limit: usize,
}

impl Default for StreamingSearchSettings {
    fn default() -> Self {
        Self {
            strategy: TargetIdentifierStrategy::ScoreGap,
            min_absolute_score: 0.3,
            min_score_gap_ratio: 0.10,
            exact_match_boost: 0.2,
            popular_tracks_limit: 5,
            albums_limit: 5,
            related_artists_limit: 5,
            other_results_limit: 20,
            top_results_limit: 10,
        }
    }
}

/// Target identification strategy for streaming search
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TargetIdentifierStrategy {
    #[default]
    ScoreGap,
}

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
    pub search: SearchSettings,
    pub catalog_store: CatalogStoreSettings,
    pub agent: AgentSettings,
    pub ingestion: IngestionSettings,
    pub related_artists: Option<RelatedArtistsSettings>,
    pub audio_analysis: Option<AudioAnalysisSettings>,
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
            enabled: dm_file.enabled.unwrap_or(true),
            max_albums_per_hour: dm_file.max_albums_per_hour.unwrap_or(10),
            max_albums_per_day: dm_file.max_albums_per_day.unwrap_or(60),
            user_max_requests_per_day: dm_file.user_max_requests_per_day.unwrap_or(100),
            user_max_queue_size: dm_file.user_max_queue_size.unwrap_or(200),
            stale_in_progress_threshold_secs: dm_file
                .stale_in_progress_threshold_secs
                .unwrap_or(3600),
            max_retries: dm_file.max_retries.unwrap_or(8),
            initial_backoff_secs: dm_file.initial_backoff_secs.unwrap_or(60),
            max_backoff_secs: dm_file.max_backoff_secs.unwrap_or(86400), // 24 hours
            backoff_multiplier: dm_file.backoff_multiplier.unwrap_or(2.5),
            audit_log_retention_days: dm_file.audit_log_retention_days.unwrap_or(90),
        };

        // Background jobs settings from file config
        let bg_jobs_file = file.background_jobs.unwrap_or_default();
        let bg_jobs_defaults = BackgroundJobsSettings::default();

        // Popular content job settings
        let pc_file = bg_jobs_file.popular_content.unwrap_or_default();
        let popular_content = PopularContentJobSettings {
            interval_hours: pc_file
                .interval_hours
                .unwrap_or(bg_jobs_defaults.popular_content.interval_hours),
            albums_limit: pc_file
                .albums_limit
                .unwrap_or(bg_jobs_defaults.popular_content.albums_limit),
            artists_limit: pc_file
                .artists_limit
                .unwrap_or(bg_jobs_defaults.popular_content.artists_limit),
            lookback_days: pc_file
                .lookback_days
                .unwrap_or(bg_jobs_defaults.popular_content.lookback_days),
            impression_lookback_days: pc_file
                .impression_lookback_days
                .unwrap_or(bg_jobs_defaults.popular_content.impression_lookback_days),
            impression_retention_days: pc_file
                .impression_retention_days
                .unwrap_or(bg_jobs_defaults.popular_content.impression_retention_days),
        };

        let cas_file = bg_jobs_file.catalog_availability_stats.unwrap_or_default();
        let catalog_availability_stats = CatalogAvailabilityStatsJobSettings {
            interval_hours: cas_file
                .interval_hours
                .unwrap_or(bg_jobs_defaults.catalog_availability_stats.interval_hours),
            startup_delay_minutes: cas_file.startup_delay_minutes.unwrap_or(
                bg_jobs_defaults
                    .catalog_availability_stats
                    .startup_delay_minutes,
            ),
        };

        // What's new batch job settings
        let wn_file = bg_jobs_file.whatsnew_batch.unwrap_or_default();
        let whatsnew_batch = IntervalJobSettings {
            interval_hours: wn_file
                .interval_hours
                .unwrap_or(bg_jobs_defaults.whatsnew_batch.interval_hours),
        };

        // Ingestion cleanup job settings (optional - only if configured)
        let ingestion_cleanup = bg_jobs_file.ingestion_cleanup.map(|ic_file| {
            let ic_defaults = IngestionCleanupJobSettings::default();
            IngestionCleanupJobSettings {
                interval_hours: ic_file.interval_hours.unwrap_or(ic_defaults.interval_hours),
                min_age_secs: ic_file.min_age_secs.unwrap_or(ic_defaults.min_age_secs),
            }
        });

        // Audit log cleanup job settings (optional - only if configured)
        let audit_log_cleanup = bg_jobs_file.audit_log_cleanup.map(|alc_file| {
            let alc_defaults = AuditLogCleanupJobSettings::default();
            AuditLogCleanupJobSettings {
                interval_hours: alc_file
                    .interval_hours
                    .unwrap_or(alc_defaults.interval_hours),
                retention_days: alc_file
                    .retention_days
                    .unwrap_or(alc_defaults.retention_days),
            }
        });

        // Device pruning job settings
        let dp_file = bg_jobs_file.device_pruning.unwrap_or_default();
        let dp_defaults = DevicePruningJobSettings::default();
        let device_pruning = DevicePruningJobSettings {
            interval_hours: dp_file.interval_hours.unwrap_or(dp_defaults.interval_hours),
            retention_days: dp_file.retention_days.unwrap_or(dp_defaults.retention_days),
        };

        let background_jobs = BackgroundJobsSettings {
            popular_content,
            catalog_availability_stats,
            whatsnew_batch,
            ingestion_cleanup,
            audit_log_cleanup,
            device_pruning,
        };

        // Catalog store settings from file config
        let catalog_store_file = file.catalog_store.unwrap_or_default();
        let catalog_store = CatalogStoreSettings {
            read_pool_size: catalog_store_file.read_pool_size.unwrap_or(4),
        };

        // Search settings from file config
        let search_file = file.search.clone().unwrap_or_default();
        let search_engine = search_file
            .engine
            .unwrap_or_else(|| "fts5-levenshtein".to_string());

        // Streaming search settings from file config
        let streaming_defaults = StreamingSearchSettings::default();
        let streaming_file = search_file.streaming.unwrap_or_default();
        let streaming = StreamingSearchSettings {
            strategy: streaming_file
                .strategy
                .map(|s| match s.to_lowercase().as_str() {
                    "score_gap" | "scoregap" => TargetIdentifierStrategy::ScoreGap,
                    _ => TargetIdentifierStrategy::ScoreGap, // default for unknown
                })
                .unwrap_or(streaming_defaults.strategy),
            min_absolute_score: streaming_file
                .min_absolute_score
                .unwrap_or(streaming_defaults.min_absolute_score),
            min_score_gap_ratio: streaming_file
                .min_score_gap_ratio
                .unwrap_or(streaming_defaults.min_score_gap_ratio),
            exact_match_boost: streaming_file
                .exact_match_boost
                .unwrap_or(streaming_defaults.exact_match_boost),
            popular_tracks_limit: streaming_file
                .popular_tracks_limit
                .unwrap_or(streaming_defaults.popular_tracks_limit),
            albums_limit: streaming_file
                .albums_limit
                .unwrap_or(streaming_defaults.albums_limit),
            related_artists_limit: streaming_file
                .related_artists_limit
                .unwrap_or(streaming_defaults.related_artists_limit),
            other_results_limit: streaming_file
                .other_results_limit
                .unwrap_or(streaming_defaults.other_results_limit),
            top_results_limit: streaming_file
                .top_results_limit
                .unwrap_or(streaming_defaults.top_results_limit),
        };

        let search = SearchSettings {
            engine: search_engine,
            streaming,
        };

        // Agent settings from file config
        let agent_file = file.agent.unwrap_or_default();
        let agent_llm_file = agent_file.llm.unwrap_or_default();
        let agent_llm_defaults = AgentLlmSettings::default();
        let agent = AgentSettings {
            enabled: agent_file.enabled.unwrap_or(false),
            max_iterations: agent_file.max_iterations.unwrap_or(20),
            llm: AgentLlmSettings {
                provider: agent_llm_file
                    .provider
                    .unwrap_or(agent_llm_defaults.provider),
                base_url: agent_llm_file
                    .base_url
                    .unwrap_or(agent_llm_defaults.base_url),
                model: agent_llm_file.model.unwrap_or(agent_llm_defaults.model),
                api_key: agent_llm_file.api_key,
                api_key_command: agent_llm_file.api_key_command,
                temperature: agent_llm_file
                    .temperature
                    .unwrap_or(agent_llm_defaults.temperature),
                timeout_secs: agent_llm_file
                    .timeout_secs
                    .unwrap_or(agent_llm_defaults.timeout_secs),
            },
        };

        // Ingestion settings from file config
        let ingestion_file = file.ingestion.unwrap_or_default();
        let ingestion_defaults = IngestionSettings::default();
        let ingestion = IngestionSettings {
            enabled: ingestion_file.enabled.unwrap_or(false),
            temp_dir: ingestion_file.temp_dir,
            max_upload_size_mb: ingestion_file
                .max_upload_size_mb
                .unwrap_or(ingestion_defaults.max_upload_size_mb),
            auto_approve_threshold: ingestion_file
                .auto_approve_threshold
                .unwrap_or(ingestion_defaults.auto_approve_threshold),
            ffmpeg_path: ingestion_file
                .ffmpeg_path
                .unwrap_or(ingestion_defaults.ffmpeg_path),
            ffprobe_path: ingestion_file
                .ffprobe_path
                .unwrap_or(ingestion_defaults.ffprobe_path),
            output_bitrate: ingestion_file
                .output_bitrate
                .unwrap_or(ingestion_defaults.output_bitrate),
        };

        // Related artists settings from file config
        let related_artists = file.related_artists.and_then(|ra| {
            if !ra.enabled.unwrap_or(false) {
                return None;
            }
            let api_key = ra.lastfm_api_key?;
            let user_agent = ra.musicbrainz_user_agent?;
            Some(RelatedArtistsSettings {
                enabled: true,
                lastfm_api_key: api_key,
                musicbrainz_user_agent: user_agent,
                batch_size: ra.batch_size.unwrap_or(50),
                similar_artists_limit: ra.similar_artists_limit.unwrap_or(20),
                interval_hours: ra.interval_hours.unwrap_or(12),
            })
        });

        // Audio analysis settings from file config
        let audio_analysis = file.audio_analysis.and_then(|aa| {
            if !aa.enabled.unwrap_or(false) {
                return None;
            }
            Some(AudioAnalysisSettings {
                enabled: true,
                interval_hours: aa.interval_hours.unwrap_or(6),
                batch_size: aa.batch_size.unwrap_or(100),
                delay_ms: aa.delay_ms.unwrap_or(500),
            })
        });

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
            search,
            catalog_store,
            agent,
            ingestion,
            related_artists,
            audio_analysis,
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

    pub fn search_db_path(&self) -> PathBuf {
        self.db_dir.join("search.db")
    }

    pub fn enrichment_db_path(&self) -> PathBuf {
        self.db_dir.join("enrichment.db")
    }

    pub fn ingestion_db_path(&self) -> PathBuf {
        self.db_dir.join("ingestion.db")
    }

    pub fn ingestion_temp_dir(&self) -> PathBuf {
        self.ingestion
            .temp_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.db_dir.join("ingestion_uploads"))
    }
}

#[derive(Debug, Clone)]
pub struct DownloadManagerSettings {
    pub enabled: bool,
    pub max_albums_per_hour: u32,
    pub max_albums_per_day: u32,
    pub user_max_requests_per_day: u32,
    pub user_max_queue_size: u32,
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
            stale_in_progress_threshold_secs: 3600,
            max_retries: 8,
            initial_backoff_secs: 60,
            max_backoff_secs: 86400, // 24 hours
            backoff_multiplier: 2.5,
            audit_log_retention_days: 90,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundJobsSettings {
    pub popular_content: PopularContentJobSettings,
    pub catalog_availability_stats: CatalogAvailabilityStatsJobSettings,
    pub whatsnew_batch: IntervalJobSettings,
    pub ingestion_cleanup: Option<IngestionCleanupJobSettings>,
    pub audit_log_cleanup: Option<AuditLogCleanupJobSettings>,
    pub device_pruning: DevicePruningJobSettings,
}

/// Settings for popular content job
#[derive(Debug, Clone)]
pub struct PopularContentJobSettings {
    pub interval_hours: u64,
    pub albums_limit: usize,
    pub artists_limit: usize,
    pub lookback_days: u32,
    pub impression_lookback_days: u32,
    pub impression_retention_days: u32,
}

impl Default for PopularContentJobSettings {
    fn default() -> Self {
        Self {
            interval_hours: 6,
            albums_limit: 20,
            artists_limit: 20,
            lookback_days: 30,
            impression_lookback_days: 365,
            impression_retention_days: 365,
        }
    }
}

/// Settings for catalog availability stats job
#[derive(Debug, Clone)]
pub struct CatalogAvailabilityStatsJobSettings {
    pub interval_hours: u64,
    pub startup_delay_minutes: u64,
}

impl Default for CatalogAvailabilityStatsJobSettings {
    fn default() -> Self {
        Self {
            interval_hours: 6,
            startup_delay_minutes: 60,
        }
    }
}

/// Settings for jobs that only need interval configuration
#[derive(Debug, Clone)]
pub struct IntervalJobSettings {
    pub interval_hours: u64,
}

impl Default for IntervalJobSettings {
    fn default() -> Self {
        Self { interval_hours: 6 }
    }
}

/// Settings for ingestion cleanup job
#[derive(Debug, Clone)]
pub struct IngestionCleanupJobSettings {
    pub interval_hours: u64,
    pub min_age_secs: u64,
}

impl Default for IngestionCleanupJobSettings {
    fn default() -> Self {
        Self {
            interval_hours: 1,
            min_age_secs: 300,
        }
    }
}

/// Settings for audit log cleanup job
#[derive(Debug, Clone)]
pub struct AuditLogCleanupJobSettings {
    pub interval_hours: u64,
    pub retention_days: u64,
}

impl Default for AuditLogCleanupJobSettings {
    fn default() -> Self {
        Self {
            interval_hours: 24,
            retention_days: 90,
        }
    }
}

/// Settings for device pruning job
#[derive(Debug, Clone)]
pub struct DevicePruningJobSettings {
    pub interval_hours: u64,
    pub retention_days: u64,
}

impl Default for DevicePruningJobSettings {
    fn default() -> Self {
        Self {
            interval_hours: 24,
            retention_days: 90,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CatalogStoreSettings {
    pub read_pool_size: usize,
}

impl Default for CatalogStoreSettings {
    fn default() -> Self {
        Self { read_pool_size: 4 }
    }
}

/// Settings for the agent LLM backend.
#[derive(Debug, Clone)]
pub struct AgentSettings {
    pub enabled: bool,
    pub max_iterations: usize,
    pub llm: AgentLlmSettings,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 20,
            llm: AgentLlmSettings::default(),
        }
    }
}

/// Settings for the LLM provider.
#[derive(Debug, Clone)]
pub struct AgentLlmSettings {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_key_command: Option<String>,
    pub temperature: f32,
    pub timeout_secs: u64,
}

impl Default for AgentLlmSettings {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            base_url: "http://localhost:11434".to_string(),
            model: "llama3.1:8b".to_string(),
            api_key: None,
            api_key_command: None,
            temperature: 0.3,
            timeout_secs: 120,
        }
    }
}

/// Settings for related artists enrichment.
#[derive(Debug, Clone)]
pub struct RelatedArtistsSettings {
    pub enabled: bool,
    pub lastfm_api_key: String,
    pub musicbrainz_user_agent: String,
    pub batch_size: usize,
    pub similar_artists_limit: usize,
    pub interval_hours: u64,
}

/// Settings for audio analysis via rustentia.
#[derive(Debug, Clone)]
pub struct AudioAnalysisSettings {
    pub enabled: bool,
    pub interval_hours: u64,
    pub batch_size: usize,
    pub delay_ms: u64,
}

/// Settings for the ingestion feature.
#[derive(Debug, Clone)]
pub struct IngestionSettings {
    pub enabled: bool,
    pub temp_dir: Option<String>,
    pub max_upload_size_mb: u64,
    pub auto_approve_threshold: f32,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub output_bitrate: String,
}

impl Default for IngestionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            temp_dir: None,
            max_upload_size_mb: 500,
            auto_approve_threshold: 0.9,
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            output_bitrate: "320k".to_string(),
        }
    }
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
    fn test_resolve_download_manager_enabled_by_default() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        // enabled defaults to true
        let config = AppConfig::resolve(&cli, None).unwrap();
        assert!(config.download_manager.enabled);
    }

    #[test]
    fn test_resolve_download_manager_explicit_disabled() {
        let temp_dir = make_temp_db_dir();
        let cli = CliConfig {
            db_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let file_config = FileConfig {
            download_manager: Some(DownloadManagerConfig {
                enabled: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };

        let config = AppConfig::resolve(&cli, Some(file_config)).unwrap();
        assert!(!config.download_manager.enabled);
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
