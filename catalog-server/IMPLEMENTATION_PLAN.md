# Detailed Implementation Plan

This document breaks down the Background Jobs System and Download Manager plans into sequential, actionable tasks.

---

## Implementation Status Summary

| Phase | Status | Progress |
|-------|--------|----------|
| Phase 0: TOML Config System | ✅ Complete | All tasks done |
| Part 1: Background Jobs System | ✅ Complete | All tasks done |
| Part 2: Download Manager | ⏳ Not Started | Planning only |

---

## Legend

- `[ ]` - Not started
- `[~]` - In progress
- `[x]` - Completed

---

## Phase 0: TOML Configuration System

**Goal:** Add a TOML-based configuration system where TOML values override CLI arguments.

### 0.1 Dependencies

- [x] **Task 0.1.1: Add `toml` and `serde` derive to Cargo.toml**

  **File:** `catalog-server/Cargo.toml`

  **Add:**
  ```toml
  toml = "0.8"
  # serde should already be present, ensure derive feature is enabled
  ```

### 0.2 Configuration Structure

- [x] **Task 0.2.1: Create `config` module**

  **Files to create:**
  - `catalog-server/src/config/mod.rs`
  - `catalog-server/src/config/file_config.rs`

  **Also add to `main.rs`:**
  ```rust
  mod config;
  use config::{AppConfig, FileConfig};
  ```

  **Also add to `cli_auth.rs`:**
  ```rust
  mod config;
  ```

- [x] **Task 0.2.2: Define `FileConfig` struct for TOML parsing**

  **Context:** This struct mirrors the TOML file structure. All fields are `Option<T>` since TOML may only partially override CLI.

  **File:** `catalog-server/src/config/file_config.rs`

  **Sample:**
  ```rust
  use serde::Deserialize;
  use std::path::Path;
  use anyhow::{Context, Result};

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
  }

  #[derive(Debug, Deserialize, Default, Clone)]
  #[serde(default)]
  pub struct BackgroundJobsConfig {
      // Future: per-job configuration can be added here
      // e.g., pub popular_content_interval_hours: Option<u64>,
  }

  impl FileConfig {
      pub fn load(path: &Path) -> Result<Self> {
          let content = std::fs::read_to_string(path)
              .with_context(|| format!("Failed to read config file: {:?}", path))?;
          toml::from_str(&content)
              .with_context(|| format!("Failed to parse config file: {:?}", path))
      }
  }
  ```

- [x] **Task 0.2.3: Define `AppConfig` struct for resolved configuration**

  **Context:** This struct holds the final resolved values (CLI defaults → TOML overrides). All fields are concrete types (not Option).

  **File:** `catalog-server/src/config/mod.rs`

  **Sample:**
  ```rust
  mod file_config;

  pub use file_config::{FileConfig, DownloadManagerConfig, BackgroundJobsConfig};

  use std::path::PathBuf;
  use crate::server::RequestsLoggingLevel;

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
      pub enabled: bool,  // true if downloader_url is set
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
  ```

- [x] **Task 0.2.4: Implement config resolution (CLI + TOML merge)**

  **Context:** TOML values override CLI values where present.

  **File:** `catalog-server/src/config/mod.rs`

  **Sample:**
  ```rust
  use anyhow::{Result, bail};

  impl AppConfig {
      pub fn resolve(cli: &CliArgs, file_config: Option<FileConfig>) -> Result<Self> {
          let file = file_config.unwrap_or_default();

          // TOML overrides CLI for each field
          let db_dir = file.db_dir
              .map(PathBuf::from)
              .or_else(|| cli.db_dir.clone())
              .ok_or_else(|| anyhow::anyhow!("db_dir must be specified via --db-dir or in config file"))?;

          // Validate db_dir exists
          if !db_dir.exists() {
              bail!("Database directory does not exist: {:?}", db_dir);
          }
          if !db_dir.is_dir() {
              bail!("db_dir is not a directory: {:?}", db_dir);
          }

          let media_path = file.media_path
              .map(PathBuf::from)
              .or_else(|| cli.media_path.clone())
              .unwrap_or_else(|| db_dir.clone());

          let port = file.port.unwrap_or(cli.port);
          let metrics_port = file.metrics_port.unwrap_or(cli.metrics_port);

          let logging_level = file.logging_level
              .and_then(|s| s.parse().ok())
              .unwrap_or(cli.logging_level.clone());

          let content_cache_age_sec = file.content_cache_age_sec.unwrap_or(cli.content_cache_age_sec);
          let frontend_dir_path = file.frontend_dir_path.or_else(|| cli.frontend_dir_path.clone());

          let downloader_url = file.downloader_url.clone()
              .or_else(|| cli.downloader_url.clone());

          let downloader_timeout_sec = file.downloader_timeout_sec.unwrap_or(cli.downloader_timeout_sec);
          let event_retention_days = file.event_retention_days.unwrap_or(cli.event_retention_days);
          let prune_interval_hours = file.prune_interval_hours.unwrap_or(cli.prune_interval_hours);

          // Download manager settings - merge file config with defaults
          let dm_file = file.download_manager.unwrap_or_default();
          let download_manager = DownloadManagerSettings {
              enabled: downloader_url.is_some(),
              max_albums_per_hour: dm_file.max_albums_per_hour.unwrap_or(10),
              max_albums_per_day: dm_file.max_albums_per_day.unwrap_or(60),
              user_max_requests_per_day: dm_file.user_max_requests_per_day.unwrap_or(100),
              user_max_queue_size: dm_file.user_max_queue_size.unwrap_or(200),
              process_interval_secs: dm_file.process_interval_secs.unwrap_or(5),
              stale_in_progress_threshold_secs: dm_file.stale_in_progress_threshold_secs.unwrap_or(3600),
              max_retries: dm_file.max_retries.unwrap_or(5),
              initial_backoff_secs: dm_file.initial_backoff_secs.unwrap_or(60),
              max_backoff_secs: dm_file.max_backoff_secs.unwrap_or(3600),
              backoff_multiplier: dm_file.backoff_multiplier.unwrap_or(2.0),
              audit_log_retention_days: dm_file.audit_log_retention_days.unwrap_or(90),
          };

          let background_jobs = BackgroundJobsSettings::default();

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
          })
      }
  }
  ```

### 0.3 CLI Integration

- [x] **Task 0.3.1: Add `--config` and `--db-dir` CLI arguments, remove positional args**

  **File:** `catalog-server/src/main.rs`

  **Before:**
  ```rust
  #[derive(Parser, Debug)]
  struct CliArgs {
      #[clap(value_parser = parse_path)]
      pub catalog_db: PathBuf,

      #[clap(value_parser = parse_path)]
      pub user_store_file_path: PathBuf,
      // ...
  }
  ```

  **After:**
  ```rust
  fn parse_dir(s: &str) -> Result<PathBuf, String> {
      let path = PathBuf::from(s);
      if !path.exists() {
          return Err(format!("Directory does not exist: {}", s));
      }
      if !path.is_dir() {
          return Err(format!("Path is not a directory: {}", s));
      }
      Ok(path)
  }

  #[derive(Parser, Debug)]
  struct CliArgs {
      /// Path to TOML configuration file. Values in the file override CLI arguments.
      #[clap(long, value_parser = parse_path)]
      pub config: Option<PathBuf>,

      /// Directory containing database files (catalog.db, user.db, server.db).
      /// Can also be specified in config file.
      #[clap(long, value_parser = parse_dir)]
      pub db_dir: Option<PathBuf>,

      // ... rest of existing args unchanged, all remain optional with defaults
  }
  ```

- [x] **Task 0.3.2: Update `main.rs` to load and resolve config**

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  let cli_args = CliArgs::parse();

  // Load TOML config if provided
  let file_config = match &cli_args.config {
      Some(path) => {
          info!("Loading configuration from {:?}", path);
          Some(FileConfig::load(path)?)
      }
      None => None,
  };

  // Resolve final configuration (TOML overrides CLI)
  let config = AppConfig::resolve(&cli_args, file_config)?;

  info!("Configuration loaded:");
  info!("  db_dir: {:?}", config.db_dir);
  info!("  media_path: {:?}", config.media_path);
  info!("  port: {}", config.port);
  info!("  download_manager.enabled: {}", config.download_manager.enabled);
  ```

- [x] **Task 0.3.3: Update all `main.rs` to use `AppConfig` instead of `CliArgs`**

  **Context:** Replace direct `cli_args.catalog_db` / `cli_args.user_store_file_path` accesses with `config.catalog_db_path()` / `config.user_db_path()`.

  **File:** `catalog-server/src/main.rs`

- [x] **Task 0.3.4: Auto-create missing database files with empty schema**

  **Context:** If `catalog.db` or `user.db` don't exist in `db_dir`, create them with empty schema.

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  // Create catalog store (will create DB if not exists)
  if !config.catalog_db_path().exists() {
      info!("Creating new catalog database at {:?}", config.catalog_db_path());
  }
  let catalog_store = Arc::new(SqliteCatalogStore::new(
      config.catalog_db_path(),
      config.media_path.clone(),
  )?);

  // Create user store (will create DB if not exists)
  if !config.user_db_path().exists() {
      info!("Creating new user database at {:?}", config.user_db_path());
  }
  let user_store = Arc::new(SqliteUserStore::new(&config.user_db_path())?);
  ```

- [x] **Task 0.3.5: Update `cli-auth` binary for new config**

  **Context:** The `cli-auth` tool currently takes an optional path. Update it to use `--db-dir` or `--config` to find `user.db`.

  **File:** `catalog-server/src/cli_auth.rs`

  **Changes:**
  - Add `--config` and `--db-dir` options similar to main.rs
  - Use `config.user_db_path()` to find the user database
  - Keep backward compatibility: if old positional path provided, use it directly

### 0.4 Docker Integration

- [x] **Task 0.4.1: Create shared Docker network configuration**

  **Context:** Add the shared network to docker-compose.yml so catalog-server can communicate with the independently-run downloader.

  **File:** `docker-compose.yml`

  **Add to networks section:**
  ```yaml
  networks:
    monitoring:
      driver: bridge
    pezzottify-internal:
      name: pezzottify-internal  # Explicit name prevents docker-compose prefix
      driver: bridge
  ```

  **Update catalog-server service:**
  ```yaml
  catalog-server:
    # ... existing config ...
    networks:
      - monitoring
      - pezzottify-internal
  ```

- [x] **Task 0.4.2: Create example TOML configuration file**

  **File:** `catalog-server/config.example.toml`

  **Content:**
  ```toml
  # Pezzottify Catalog Server Configuration
  # Copy this file to config.toml and customize for your deployment.
  # Values here override CLI arguments.

  # Core settings
  db_dir = "/data/db"
  media_path = "/data/media"
  port = 3001
  metrics_port = 9091
  logging_level = "path"
  content_cache_age_sec = 60
  frontend_dir_path = "/app/web"

  # Downloader service URL (enables download manager when set)
  # The downloader should be on the same Docker network (pezzottify-internal)
  # downloader_url = "http://downloader:3002"
  downloader_timeout_sec = 300

  # Event pruning
  event_retention_days = 30
  prune_interval_hours = 24

  # Download Manager settings (only used if downloader_url is set)
  [download_manager]
  max_albums_per_hour = 10
  max_albums_per_day = 60
  user_max_requests_per_day = 100
  user_max_queue_size = 200
  process_interval_secs = 5
  stale_in_progress_threshold_secs = 3600
  max_retries = 5
  initial_backoff_secs = 60
  max_backoff_secs = 3600
  backoff_multiplier = 2.0
  audit_log_retention_days = 90

  # Background Jobs settings
  [background_jobs]
  # Future: per-job configuration
  ```

- [x] **Task 0.4.3: Update docker-compose.yml to use config file**

  **File:** `docker-compose.yml`

  **Change catalog-server service:**
  ```yaml
  catalog-server:
    build:
      context: .
      dockerfile: catalog-server/Dockerfile
      args:
        GIT_HASH: ${GIT_HASH:-unknown}
        GIT_DIRTY: ${GIT_DIRTY:-0}
    ports:
      - "3001:3001"
    expose:
      - "9091"
    volumes:
      - /home/lelloman/pezzottify-catalog/:/data/db
      - /home/lelloman/pezzottify-catalog/:/data/media
      - ./catalog-server/config.toml:/etc/pezzottify/config.toml:ro
    networks:
      - monitoring
      - pezzottify-internal
    environment:
      - LOG_LEVEL=info
    restart: unless-stopped
    command: [ "catalog-server", "--config", "/etc/pezzottify/config.toml" ]
  ```

- [x] **Task 0.4.4: Add config.toml to .gitignore**

  **File:** `.gitignore`

  **Add:**
  ```
  catalog-server/config.toml
  ```

### 0.5 Documentation

- [x] **Task 0.5.1: Update `catalog-server/README.md` with config documentation**

  **Context:** Document the TOML config system, precedence rules, `--db-dir`, and all available options.

  **File:** `catalog-server/README.md`

- [x] **Task 0.5.2: Update `CLAUDE.md` with new config approach**

  **Context:** Update the "Running the server" section with new CLI format.

  **File:** `CLAUDE.md`

  **Sample change:**
  ```markdown
  **Running the server:**
  ```bash
  cd catalog-server
  # Using config file (recommended):
  cargo run -- --config ./config.toml

  # Using CLI arguments:
  cargo run -- --db-dir /path/to/db-dir --media-path /path/to/media --port 3001
  ```

### 0.6 Tests

- [x] **Task 0.6.1: Add unit tests for config resolution**

  **Context:** Test CLI-only, TOML-only, and CLI+TOML merge scenarios.

  **File:** `catalog-server/src/config/mod.rs` (test module)

  **Test cases:**
  - CLI only: all values from CLI
  - TOML only: all values from file, CLI has defaults
  - TOML overrides CLI: file values take precedence
  - Missing required field: error when db_dir not provided
  - Invalid db_dir: error when path doesn't exist or isn't a directory
  - Download manager enabled/disabled based on downloader_url

---

## Part 1: Background Jobs System

### Phase 1: CLI Refactoring (Database Directory)

**Note:** Most CLI refactoring is handled in Phase 0. This phase covers remaining cleanup.

#### 1.1 Remaining CLI Changes

- [x] **Task 1.1.1: Verify `SqliteCatalogStore` and `SqliteUserStore` create DB if not exists**

  **Context:** Both stores should create their database file with proper schema if the file doesn't exist.

  **Files to check:**
  - `catalog-server/src/catalog_store/store.rs`
  - `catalog-server/src/user/sqlite_user_store.rs`

#### 1.2 Tests

- [x] **Task 1.2.1: Update integration tests that spawn the server**

  **Context:** Any tests that start the server with CLI arguments need updating.

  **Files:** Search for tests that use catalog-server CLI arguments in spawned processes.

---

### Phase 2: Background Jobs System

#### 2.1 Permission Rename

**Goal:** Rename `RebootServer` permission to `ServerAdmin`.

- [x] **Task 2.1.1: Update permission enum in `permissions.rs`**

  **Context:** Rename the variant and update any associated strings. The int value (7) stays the same for backward compatibility.

  **File:** `catalog-server/src/user/permissions.rs`

  **Before:** `RebootServer,` (line 12)
  **After:** `ServerAdmin,`

  Also update:
  - `to_int()`: `Permission::RebootServer => 7` → `Permission::ServerAdmin => 7`
  - `from_int()`: `7 => Some(Permission::RebootServer)` → `7 => Some(Permission::ServerAdmin)`
  - `ADMIN_PERMISSIONS` array

- [x] **Task 2.1.2: Update all Rust code references to `RebootServer`**

  **Context:** Find and replace all uses. Based on grep, these files need updates:

  **Files:**
  - `catalog-server/src/server/session.rs` (line 209)
  - `catalog-server/src/server/server.rs` (lines 159, 162, 163, 2153, 2922, 3703, 3708, 3744, 3753)
  - `catalog-server/src/user/sqlite_user_store.rs` (line 3267)
  - `catalog-server/src/user/permissions.rs` (multiple test assertions)

- [x] **Task 2.1.3: Update `catalog-server/README.md` permission documentation**

  **File:** `catalog-server/README.md`

  **Changes:**
  - Line 83: `RebootServer` → `ServerAdmin`
  - Line 427: Update description
  - Line 509: Update in admin permissions list

- [x] **Task 2.1.4: Update web frontend permission references**

  **Files and changes:**
  - `web/src/store/user.js` line 9: `'RebootServer'` → `'ServerAdmin'` in `ADMIN_PERMISSIONS`
  - `web/src/store/user.js` line 404: `canRebootServer` → `canServerAdmin`, update string
  - `web/src/store/user.js` line 517: export name change
  - `web/src/store/remote.js` line 471: update comment
  - `web/src/views/AdminView.vue` line 68: `'RebootServer'` → `'ServerAdmin'`
  - `web/src/components/admin/UserManagement.vue` line 263: `'RebootServer'` → `'ServerAdmin'`

- [x] **Task 2.1.5: Update Android permission references**

  **Files and changes:**
  - `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/model/Permission.kt`:
    - Line 13: `RebootServer` → `ServerAdmin`
    - Line 24: Update display name
    - Line 36: Update description
  - `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`:
    - Line 45: `RebootServer` → `ServerAdmin`
  - `android/app/src/main/java/com/lelloman/pezzottify/android/ui/InteractorsModule.kt`:
    - Line 806: Update mapping

#### 2.2 Server Database and Store

**Goal:** Create `server.db` for job execution history and schedule state.

- [x] **Task 2.2.1: Create `server_store` module structure**

  **Context:** New module for server operational state persistence.

  **Files to create:**
  - `catalog-server/src/server_store/mod.rs`
  - `catalog-server/src/server_store/sqlite_server_store.rs`
  - `catalog-server/src/server_store/models.rs`
  - `catalog-server/src/server_store/schema.rs`

  **Add to `main.rs`:**
  ```rust
  mod server_store;
  use server_store::SqliteServerStore;
  ```

- [x] **Task 2.2.2: Define `JobRun` and `JobScheduleState` models**

  **Context:** Data structures matching the `job_runs` and `job_schedules` tables.

  **File:** `catalog-server/src/server_store/models.rs`

  **Sample:**
  ```rust
  use chrono::{DateTime, Utc};

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum JobRunStatus {
      Running,
      Completed,
      Failed,
  }

  impl JobRunStatus {
      pub fn as_str(&self) -> &'static str {
          match self {
              JobRunStatus::Running => "running",
              JobRunStatus::Completed => "completed",
              JobRunStatus::Failed => "failed",
          }
      }

      pub fn from_str(s: &str) -> Option<Self> {
          match s {
              "running" => Some(JobRunStatus::Running),
              "completed" => Some(JobRunStatus::Completed),
              "failed" => Some(JobRunStatus::Failed),
              _ => None,
          }
      }
  }

  #[derive(Debug, Clone)]
  pub struct JobRun {
      pub id: i64,
      pub job_id: String,
      pub started_at: DateTime<Utc>,
      pub finished_at: Option<DateTime<Utc>>,
      pub status: JobRunStatus,
      pub error_message: Option<String>,
      pub triggered_by: String,  // "schedule", "hook:OnStartup", "manual", etc.
  }

  #[derive(Debug, Clone)]
  pub struct JobScheduleState {
      pub job_id: String,
      pub next_run_at: DateTime<Utc>,
      pub last_run_at: Option<DateTime<Utc>>,
  }
  ```

- [x] **Task 2.2.3: Define `ServerStore` trait**

  **Context:** Interface for job history operations. Note: Using synchronous trait (matching existing patterns in codebase).

  **File:** `catalog-server/src/server_store/mod.rs`

  **Sample:**
  ```rust
  mod models;
  mod schema;
  mod sqlite_server_store;

  pub use models::*;
  pub use sqlite_server_store::SqliteServerStore;

  use anyhow::Result;

  pub trait ServerStore: Send + Sync {
      fn record_job_start(&self, job_id: &str, triggered_by: &str) -> Result<i64>;
      fn record_job_finish(&self, run_id: i64, status: JobRunStatus, error_message: Option<String>) -> Result<()>;
      fn get_running_jobs(&self) -> Result<Vec<JobRun>>;
      fn get_job_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobRun>>;
      fn get_last_run(&self, job_id: &str) -> Result<Option<JobRun>>;
      fn mark_stale_jobs_failed(&self) -> Result<usize>;

      // Schedule state
      fn get_schedule_state(&self, job_id: &str) -> Result<Option<JobScheduleState>>;
      fn update_schedule_state(&self, state: &JobScheduleState) -> Result<()>;
      fn get_all_schedule_states(&self) -> Result<Vec<JobScheduleState>>;
  }
  ```

- [x] **Task 2.2.4: Create schema for `server.db`**

  **File:** `catalog-server/src/server_store/schema.rs`

  **Sample:**
  ```rust
  use crate::sqlite_persistence::VersionedSchema;

  pub const SERVER_VERSIONED_SCHEMAS: &[VersionedSchema] = &[
      VersionedSchema {
          version: 1,
          up: r#"
              CREATE TABLE job_runs (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  job_id TEXT NOT NULL,
                  started_at TEXT NOT NULL,
                  finished_at TEXT,
                  status TEXT NOT NULL,
                  error_message TEXT,
                  triggered_by TEXT NOT NULL
              );

              CREATE INDEX idx_job_runs_job_id_started ON job_runs(job_id, started_at DESC);
              CREATE INDEX idx_job_runs_status ON job_runs(status);

              CREATE TABLE job_schedules (
                  job_id TEXT PRIMARY KEY,
                  next_run_at TEXT NOT NULL,
                  last_run_at TEXT
              );
          "#,
      },
  ];
  ```

- [x] **Task 2.2.5: Implement `SqliteServerStore`**

  **Context:** SQLite implementation of `ServerStore`.

  **File:** `catalog-server/src/server_store/sqlite_server_store.rs`

- [x] **Task 2.2.6: Add unit tests for `SqliteServerStore`**

  **Context:** Test CRUD operations, state transitions, stale job handling.

  **File:** `catalog-server/src/server_store/sqlite_server_store.rs` (test module)

#### 2.3 Job Scheduler Core

**Goal:** Implement the job scheduling and execution system.

- [x] **Task 2.3.1: Create `background_jobs` module structure**

  **Files to create:**
  - `catalog-server/src/background_jobs/mod.rs`
  - `catalog-server/src/background_jobs/job.rs` (trait + enums)
  - `catalog-server/src/background_jobs/scheduler.rs`
  - `catalog-server/src/background_jobs/context.rs`
  - `catalog-server/src/background_jobs/jobs/mod.rs` (for specific job implementations)

  **Add to `main.rs`:**
  ```rust
  mod background_jobs;
  use background_jobs::{JobScheduler, HookEvent};
  ```

- [x] **Task 2.3.2: Define `BackgroundJob` trait and related types**

  **File:** `catalog-server/src/background_jobs/job.rs`

  **Sample:**
  ```rust
  use std::time::Duration;
  use anyhow::Result;
  use super::context::JobContext;

  #[derive(Debug, Clone)]
  pub enum JobSchedule {
      /// Run at specific times using cron syntax
      Cron(String),
      /// Run at fixed intervals
      Interval(Duration),
      /// Run only in response to hooks
      Hook(HookEvent),
      /// Combination of scheduled and hook-triggered
      Combined {
          cron: Option<String>,
          interval: Option<Duration>,
          hooks: Vec<HookEvent>,
      },
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum HookEvent {
      OnStartup,
      OnCatalogChange,
      OnUserCreated,
      OnDownloadComplete,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ShutdownBehavior {
      /// Job can be cancelled immediately
      Cancellable,
      /// Wait for job to complete before shutdown
      WaitForCompletion,
  }

  #[derive(Debug)]
  pub enum JobError {
      NotFound,
      AlreadyRunning,
      ExecutionFailed(String),
      Cancelled,
      Timeout,
  }

  impl std::fmt::Display for JobError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
              JobError::NotFound => write!(f, "Job not found"),
              JobError::AlreadyRunning => write!(f, "Job is already running"),
              JobError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
              JobError::Cancelled => write!(f, "Job was cancelled"),
              JobError::Timeout => write!(f, "Job timed out"),
          }
      }
  }

  impl std::error::Error for JobError {}

  /// Trait for background jobs.
  /// Jobs are synchronous - long-running work should spawn tasks internally.
  pub trait BackgroundJob: Send + Sync {
      fn id(&self) -> &'static str;
      fn name(&self) -> &'static str;
      fn description(&self) -> &'static str;
      fn schedule(&self) -> JobSchedule;
      fn shutdown_behavior(&self) -> ShutdownBehavior;

      /// Execute the job. Called from a blocking context.
      fn execute(&self, ctx: &JobContext) -> Result<(), JobError>;
  }
  ```

- [x] **Task 2.3.3: Define `JobContext` struct**

  **File:** `catalog-server/src/background_jobs/context.rs`

  **Sample:**
  ```rust
  use std::sync::Arc;
  use tokio_util::sync::CancellationToken;
  use crate::catalog_store::CatalogStore;
  use crate::user::SqliteUserStore;
  use crate::server_store::ServerStore;

  #[derive(Clone)]
  pub struct JobContext {
      pub cancellation_token: CancellationToken,
      pub catalog_store: Arc<dyn CatalogStore>,
      pub user_store: Arc<SqliteUserStore>,
      pub server_store: Arc<dyn ServerStore>,
      // Add more dependencies as needed
  }

  impl JobContext {
      pub fn is_cancelled(&self) -> bool {
          self.cancellation_token.is_cancelled()
      }
  }
  ```

- [x] **Task 2.3.4: Implement `JobScheduler` struct**

  **File:** `catalog-server/src/background_jobs/scheduler.rs`

  **Key components:**
  - Job registry (HashMap of job_id -> Arc<dyn BackgroundJob>)
  - Running jobs tracking (HashMap of job_id -> JoinHandle)
  - Hook event channel (mpsc receiver)
  - Server store reference for persistence

  **Key methods:**
  - `new()` - Initialize with server store
  - `register_job()` - Add a job to the registry
  - `trigger_job()` - Manually trigger a job
  - `run()` - Main scheduler loop
  - `shutdown()` - Graceful shutdown

- [x] **Task 2.3.5: Implement scheduler main loop**

  **File:** `catalog-server/src/background_jobs/scheduler.rs`

  **Sample:**
  ```rust
  pub async fn run(&mut self) {
      // On startup: mark stale running jobs as failed
      if let Err(e) = self.server_store.mark_stale_jobs_failed() {
          error!("Failed to mark stale jobs: {}", e);
      }

      // Fire OnStartup hooks
      self.trigger_jobs_for_hook(HookEvent::OnStartup).await;

      loop {
          let sleep_duration = self.time_until_next_scheduled_job();

          tokio::select! {
              _ = tokio::time::sleep(sleep_duration) => {
                  self.run_due_jobs().await;
              }
              Some(event) = self.hook_receiver.recv() => {
                  self.trigger_jobs_for_hook(event).await;
              }
              _ = self.shutdown_signal.cancelled() => {
                  info!("Scheduler received shutdown signal");
                  self.shutdown().await;
                  break;
              }
          }
      }
  }
  ```

- [x] **Task 2.3.6: Add hook event dispatch channel to server state**

  **File:** `catalog-server/src/server/mod.rs` (or wherever ServerState is)

  **Add:**
  ```rust
  use tokio::sync::mpsc;
  use crate::background_jobs::HookEvent;

  pub struct ServerState {
      // ... existing fields ...
      pub hook_sender: mpsc::Sender<HookEvent>,
  }
  ```

- [x] **Task 2.3.7: Implement graceful shutdown handling**

  **File:** `catalog-server/src/background_jobs/scheduler.rs`

  **Sample:**
  ```rust
  async fn shutdown(&mut self) {
      info!("Shutting down scheduler...");

      // Cancel cancellable jobs
      for (job_id, handle) in &self.running_jobs {
          let job = self.jobs.get(job_id);
          if let Some(job) = job {
              if job.shutdown_behavior() == ShutdownBehavior::Cancellable {
                  // Cancel via token
                  if let Some(token) = self.job_cancel_tokens.get(job_id) {
                      token.cancel();
                  }
              }
          }
      }

      // Wait for WaitForCompletion jobs
      for (job_id, handle) in self.running_jobs.drain() {
          let job = self.jobs.get(&job_id);
          if let Some(job) = job {
              if job.shutdown_behavior() == ShutdownBehavior::WaitForCompletion {
                  info!("Waiting for job {} to complete...", job_id);
                  let _ = handle.await;
              }
          }
      }

      info!("Scheduler shutdown complete");
  }
  ```

- [x] **Task 2.3.8: Add unit tests for scheduler**

  **File:** `catalog-server/src/background_jobs/scheduler.rs` (test module)

#### 2.4 Admin API

**Goal:** REST endpoints for listing and triggering jobs.

- [x] **Task 2.4.1: Create `GET /v1/admin/jobs` handler**

  **Context:** Returns all registered jobs with current state.

  **File:** `catalog-server/src/server/server.rs`

  **Response format:**
  ```json
  {
    "jobs": [
      {
        "id": "popular_content",
        "name": "Popular Content",
        "description": "Pre-compute popular content cache",
        "schedule": { "type": "interval", "value_secs": 86400 },
        "status": "idle",
        "last_run": {
          "started_at": "2024-01-15T10:00:00Z",
          "finished_at": "2024-01-15T10:00:05Z",
          "status": "completed"
        },
        "next_run_at": "2024-01-16T10:00:00Z"
      }
    ]
  }
  ```

- [x] **Task 2.4.2: Create `POST /v1/admin/jobs/:job_id/trigger` handler**

  **Context:** Manually trigger a job. Returns 409 if already running.

  **File:** `catalog-server/src/server/server.rs`

- [x] **Task 2.4.3: Add `require_server_admin` middleware**

  **Context:** Rename/update existing `require_reboot_server` middleware to `require_server_admin`.

  **File:** `catalog-server/src/server/server.rs`

- [x] **Task 2.4.4: Wire routes into router**

  **File:** `catalog-server/src/server/server.rs`

  **Add:**
  ```rust
  .route("/v1/admin/jobs", get(list_jobs))
  .route("/v1/admin/jobs/:job_id/trigger", post(trigger_job))
  ```

#### 2.5 Main Integration

**Goal:** Initialize and run the scheduler alongside the HTTP server.

- [x] **Task 2.5.1: Initialize `SqliteServerStore` in main.rs**

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  let server_store = Arc::new(SqliteServerStore::new(&config.server_db_path())?);
  ```

- [x] **Task 2.5.2: Initialize `JobScheduler` with registered jobs**

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  let (hook_sender, hook_receiver) = tokio::sync::mpsc::channel(100);
  let shutdown_token = CancellationToken::new();

  let job_context = JobContext {
      cancellation_token: shutdown_token.child_token(),
      catalog_store: catalog_store.clone(),
      user_store: user_store.clone(),
      server_store: server_store.clone(),
  };

  let mut scheduler = JobScheduler::new(
      server_store.clone(),
      hook_receiver,
      shutdown_token.clone(),
      job_context,
  );

  // Register jobs
  scheduler.register_job(Arc::new(PopularContentJob::new()));
  ```

- [x] **Task 2.5.3: Run scheduler with `tokio::select!` alongside HTTP server**

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  tokio::select! {
      result = run_server(config, catalog_store, user_store, hook_sender) => {
          info!("HTTP server stopped: {:?}", result);
          shutdown_token.cancel();
      },
      _ = scheduler.run() => {
          info!("Scheduler stopped");
      },
      _ = tokio::signal::ctrl_c() => {
          info!("Received Ctrl+C, initiating shutdown");
          shutdown_token.cancel();
      }
  }
  ```

#### 2.6 Initial Job: Popular Content

**Goal:** Implement the first background job as a reference implementation.

- [x] **Task 2.6.1: Create `popular_content` job**

  **File:** `catalog-server/src/background_jobs/jobs/popular_content.rs`

- [x] **Task 2.6.2: Register `popular_content` job in scheduler**

  **File:** `catalog-server/src/main.rs`

- [x] **Task 2.6.3: Add tests for `popular_content` job**

  **File:** `catalog-server/src/background_jobs/jobs/popular_content.rs`

#### 2.7 Metrics

- [x] **Task 2.7.1: Add Prometheus metrics for job execution**

  **File:** `catalog-server/src/server/metrics.rs`

  **Metrics:**
  - `pezzottify_background_job_executions_total{job_id, status}`
  - `pezzottify_background_job_duration_seconds{job_id}`
  - `pezzottify_background_job_running{job_id}`

- [x] **Task 2.7.2: Emit metrics from scheduler**

  **File:** `catalog-server/src/background_jobs/scheduler.rs`

---

## Part 2: Download Manager

**Prerequisites:** Background Jobs System (Phase 2)

**Reference:** See `DOWNLOAD_MANAGER_PLAN.md` for full design documentation.

### Overview

A queue-based asynchronous download manager that handles content downloads from an external downloader service. Three main components:

1. **User Content Requests** - Users with `RequestContent` permission can search and request downloads
2. **Catalog Integrity Watchdog** - Daily scan for missing files, auto-queues repairs
3. **Catalog Expansion Agent** - Smart expansion based on listening stats (Phase 2, deferred)

### External Downloader Service API

The download manager integrates with an external downloader service via HTTP. Full API documentation: `/home/lelloman/pezzottify-downloader/API.md`

**Base URL:** Configured via `downloader_url` (e.g., `http://downloader:8080`)

#### Metadata Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /search?q=...&type=album,artist&limit=N` | Search catalog |
| `GET /album/:id` | Album metadata (name, artists, covers, discs with track IDs) |
| `GET /album/:id/tracks` | All track metadata for album |
| `GET /artist/:id` | Artist metadata (name, portraits) |
| `GET /track/:id` | Single track metadata |

#### Download Endpoints (Raw Bytes)

| Endpoint | Response | Description |
|----------|----------|-------------|
| `GET /track/:id/audio` | `audio/ogg`, `audio/mpeg`, etc. | Track audio file |
| `GET /image/:id` | `image/jpeg` | Cover art or artist portrait |

#### ID Formats

- **Content IDs (base62):** 22-character alphanumeric (e.g., `4u7EnebtmKWzUH433cf5Qv`) - used for tracks, albums, artists
- **File IDs (hex):** 40-character lowercase hex (e.g., `2c9a122b0f6a1c6f083b1725f309d3a25636f4ae`) - used for audio files and images

#### Rate Limiting

- Only one audio download at a time (returns 429 if busy)
- Metadata requests have no rate limit

#### Error Responses

```json
{
  "error": "ERROR_CODE",
  "code": "ERROR_CODE",
  "message": "Human readable error message"
}
```

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `NOT_FOUND` | 404 | Resource not found |
| `BAD_REQUEST` | 400 | Invalid ID format |
| `TIMEOUT` | 504 | Request timed out |
| `TOO_MANY_REQUESTS` | 429 | Server busy (audio download in progress) |
| `SERVICE_UNAVAILABLE` | 503 | Downloader not ready |

### Parent-Child Queue Model

Album downloads are broken into parent and child queue items for explicit tracking and easy recovery.

**Why parent-child:**
- Each download is atomic and independently retryable
- Progress is explicit: "8/12 tracks downloaded"
- Recovery is trivial: on restart, just process remaining PENDING children
- Same model works for album requests, watchdog repairs, and future single-track requests

**Flow:**

```
1. User requests album ABC
   → QueueItem(id=1, type=Album, parent_id=null, status=PENDING)

2. Processor picks up item 1:
   - Fetch metadata: GET /album/:id, GET /album/:id/tracks, GET /artist/:id
   - Create catalog entries (artists, album, tracks - without audio files)
   - Create children:
     → QueueItem(id=2, type=AlbumImage, parent_id=1, content_id=img1)
     → QueueItem(id=3, type=ArtistImage, parent_id=1, content_id=img2)
     → QueueItem(id=4, type=TrackAudio, parent_id=1, content_id=track1)
     → QueueItem(id=5, type=TrackAudio, parent_id=1, content_id=track2)
     → ...
   - Item 1 status → IN_PROGRESS

3. Children processed individually:
   - GET /image/:id → write to {media_path}/images/{id}.jpg → COMPLETED
   - GET /track/:id/audio → write to {media_path}/audio/{id}.{ext} → COMPLETED
   - Failed downloads → RETRY_WAITING or FAILED

4. When all children reach terminal state:
   - All COMPLETED → parent COMPLETED
   - Any FAILED → parent FAILED (partial album, watchdog can retry failed tracks later)
```

**Child items inherit from parent:**
- Priority
- Request source
- User ID (for audit trail)

**User's "my requests" view:** Shows only `parent_id IS NULL` items. Can expand to see children/progress.

**Watchdog repairs:** Creates items with `parent_id = null` and `source = Watchdog`. These are standalone, not tied to a user request.

### State Machine

```
┌─────────┐
│ PENDING │ ← Initial state (newly queued)
└────┬────┘
     │
     ↓ (processor picks up by priority)
┌──────────────┐
│ IN_PROGRESS  │ ← Currently downloading
└──────┬───────┘
       │
       ├─→ Success ──────────────────→ [COMPLETED] (terminal)
       │
       └─→ Failure
           │
           ├─→ retry_count < max_retries
           │   │
           │   ↓
           │   ┌───────────────┐
           │   │ RETRY_WAITING │ ← Exponential backoff
           │   └───────┬───────┘
           │           │
           │           └─→ (after backoff) → PENDING
           │
           └─→ retry_count >= max_retries → [FAILED] (terminal)
```

### Priority System

| Priority | Value | Source | Description |
|----------|-------|--------|-------------|
| Highest | 1 | Watchdog | Fix missing files in existing catalog |
| Medium | 2 | User | User-initiated requests |
| Lowest | 3 | Expansion | Smart catalog growth (Phase 2) |

Queue processing order: `ORDER BY priority ASC, created_at ASC`

### Design Notes

**ID System:** External content IDs from the music provider ARE our catalog IDs. No mapping needed - duplicate detection is a direct catalog lookup.

**Content ID Types in Queue:**
- `Album`, `TrackAudio`: `content_id` is base62 ID (22 chars, e.g., `4u7EnebtmKWzUH433cf5Qv`)
- `AlbumImage`, `ArtistImage`: `content_id` is hex ID (40 chars, e.g., `2c9a122b0f6a1c6f...`) from `covers[].id` or `portraits[].id`

**Discography Requests:** Best-effort processing. If some albums fail rate limits or are already in catalog, the rest are still queued. Response includes `albums_queued` and `albums_skipped` counts.

**User Cancellation:** Not supported. Users cannot cancel requests once submitted. With timeouts in place and low volume, this simplifies the API without meaningful loss. Admins can manually intervene if needed.

**Stale In-Progress Handling:** Items stuck in IN_PROGRESS beyond the threshold trigger alerts (metrics + warning logs) rather than automatic failure. This signals human intervention is needed rather than hiding underlying issues.

**Queue Position:** `get_queue_position` returns the count of ALL items ahead in the queue (across all priorities), not just same-priority items. Returns `None` if item not found or in terminal state.

---

### Phase DM-1: Core Infrastructure

#### DM-1.1 Module Structure

- [x] **Task DM-1.1.1: Create `download_manager` module**

  **Files to create:**
  - `catalog-server/src/download_manager/mod.rs`
  - `catalog-server/src/download_manager/models.rs`
  - `catalog-server/src/download_manager/queue_store.rs`
  - `catalog-server/src/download_manager/audit_logger.rs`
  - `catalog-server/src/download_manager/retry_policy.rs`
  - `catalog-server/src/download_manager/schema.rs`
  - `catalog-server/src/download_manager/downloader_client.rs`
  - `catalog-server/src/download_manager/downloader_types.rs`
  - `catalog-server/src/download_manager/search_proxy.rs`
  - `catalog-server/src/download_manager/catalog_ingestion.rs`
  - `catalog-server/src/download_manager/job_processor.rs`
  - `catalog-server/src/download_manager/watchdog.rs`
  - `catalog-server/src/download_manager/manager.rs`

  **Add to `main.rs`:**
  ```rust
  mod download_manager;
  use download_manager::DownloadManager;
  ```

#### DM-1.2 Models

- [x] **Task DM-1.2.1: Define queue enums**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
  pub enum QueueStatus {
      Pending,
      InProgress,
      RetryWaiting,
      Completed,    // terminal
      Failed,       // terminal
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  pub enum QueuePriority {
      Watchdog = 1,   // Highest priority - integrity repairs
      User = 2,       // User requests
      Expansion = 3,  // Auto-expansion, discography fills
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
  pub enum DownloadContentType {
      Album,          // Full album (metadata + tracks + audio + images)
      TrackAudio,     // Single track audio file
      ArtistImage,    // Artist image
      AlbumImage,     // Album cover art
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
  pub enum RequestSource {
      User,       // Explicit user request
      Watchdog,   // Integrity watchdog repair
      Expansion,  // Auto-expansion (e.g., related content)
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum DownloadErrorType {
      Connection,  // Network error - retry
      Timeout,     // Request timeout - retry
      NotFound,    // Content not found - NO retry (immediate fail)
      Parse,       // Response parse error - retry
      Storage,     // File system error - retry
      Unknown,     // Unknown error - retry
  }
  ```

- [x] **Task DM-1.2.2: Define `QueueItem` struct**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Clone)]
  pub struct QueueItem {
      pub id: String,                          // UUID
      pub parent_id: Option<String>,           // Parent queue item ID (for child items)
      pub status: QueueStatus,
      pub priority: QueuePriority,
      pub content_type: DownloadContentType,
      pub content_id: String,                  // External ID from music provider
      pub content_name: Option<String>,        // For display (album/artist name)
      pub artist_name: Option<String>,         // For display
      pub request_source: RequestSource,
      pub requested_by_user_id: Option<String>,
      pub created_at: i64,                     // Unix timestamp
      pub started_at: Option<i64>,             // When IN_PROGRESS started
      pub completed_at: Option<i64>,           // When reached terminal state
      pub last_attempt_at: Option<i64>,        // Last attempt timestamp
      pub next_retry_at: Option<i64>,          // When to retry (for RETRY_WAITING)
      pub retry_count: i32,
      pub max_retries: i32,
      pub error_type: Option<DownloadErrorType>,
      pub error_message: Option<String>,
      pub bytes_downloaded: Option<u64>,
      pub processing_duration_ms: Option<i64>,
  }
  ```

- [x] **Task DM-1.2.3: Define `UserRequestView` struct**

  **File:** `catalog-server/src/download_manager/models.rs`

  **Context:** Simplified view for user-facing API responses. Includes child progress for album requests.

  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct UserRequestView {
      pub id: String,
      pub content_type: DownloadContentType,
      pub content_id: String,
      pub content_name: String,        // Album/artist name for display
      pub artist_name: Option<String>,
      pub status: QueueStatus,
      pub created_at: i64,
      pub completed_at: Option<i64>,
      pub error_message: Option<String>,
      pub progress: Option<DownloadProgress>,  // For album requests
      pub queue_position: Option<usize>,  // Position in queue (for pending items)
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct DownloadProgress {
      pub total_children: usize,
      pub completed: usize,
      pub failed: usize,
      pub pending: usize,
      pub in_progress: usize,
  }
  ```

- [x] **Task DM-1.2.4: Define audit types**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
  pub enum AuditEventType {
      RequestCreated,        // User submitted album request
      DownloadStarted,       // Item claimed for processing
      ChildrenCreated,       // Album spawned child items (includes count in details)
      DownloadCompleted,     // Item finished successfully (child binary or parent all-children-done)
      DownloadFailed,        // Item failed (after max retries)
      RetryScheduled,        // Item scheduled for retry
      AdminRetry,            // Admin reset failed item to pending
      WatchdogQueued,        // Watchdog queued repair item
      WatchdogScanStarted,
      WatchdogScanCompleted,
  }

  #[derive(Debug, Clone)]
  pub struct AuditLogEntry {
      pub id: i64,
      pub timestamp: i64,
      pub event_type: AuditEventType,
      pub queue_item_id: Option<String>,
      pub content_type: Option<DownloadContentType>,
      pub content_id: Option<String>,
      pub user_id: Option<String>,
      pub request_source: Option<RequestSource>,
      pub details: Option<serde_json::Value>,  // Event-specific JSON data
  }

  #[derive(Debug, Clone, Default)]
  pub struct AuditLogFilter {
      pub queue_item_id: Option<String>,
      pub user_id: Option<String>,
      pub event_types: Option<Vec<AuditEventType>>,
      pub content_type: Option<DownloadContentType>,
      pub content_id: Option<String>,
      pub since: Option<i64>,
      pub until: Option<i64>,
      pub limit: usize,
      pub offset: usize,
  }
  ```

- [x] **Task DM-1.2.5: Define statistics and status types**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct UserLimitStatus {
      pub requests_today: i32,
      pub max_per_day: i32,
      pub in_queue: i32,
      pub max_queue: i32,
      pub can_request: bool,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct CapacityStatus {
      pub albums_this_hour: i32,
      pub max_per_hour: i32,
      pub albums_today: i32,
      pub max_per_day: i32,
      pub at_capacity: bool,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct QueueStats {
      pub pending: i64,
      pub in_progress: i64,
      pub retry_waiting: i64,
      pub completed_today: i64,
      pub failed_today: i64,
  }

  #[derive(Debug, Clone)]
  pub struct ActivityLogEntry {
      pub hour_bucket: i64,        // Unix timestamp truncated to hour
      pub albums_downloaded: i64,
      pub tracks_downloaded: i64,
      pub images_downloaded: i64,
      pub bytes_downloaded: i64,
      pub failed_count: i64,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct HourlyCounts {
      pub albums: i64,
      pub tracks: i64,
      pub images: i64,
      pub bytes: i64,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct DailyCounts {
      pub albums: i64,
      pub tracks: i64,
      pub images: i64,
      pub bytes: i64,
  }

  #[derive(Debug, Clone)]
  pub struct ProcessingResult {
      pub queue_item_id: String,
      pub content_type: DownloadContentType,
      pub success: bool,
      pub bytes_downloaded: Option<u64>,
      pub duration_ms: i64,
      pub error: Option<DownloadError>,
  }

  #[derive(Debug, Clone)]
  pub struct DownloadError {
      pub error_type: DownloadErrorType,
      pub message: String,
  }

  impl DownloadError {
      pub fn is_retryable(&self) -> bool {
          !matches!(self.error_type, DownloadErrorType::NotFound)
      }
  }
  ```

- [x] **Task DM-1.2.6: Define search result types**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SearchType {
      Album,
      Artist,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct SearchResult {
      pub id: String,              // External ID
      #[serde(rename = "type")]
      pub result_type: String,     // "album" or "artist"
      pub name: String,
      pub artist_name: Option<String>,
      pub image_url: Option<String>,
      pub year: Option<i32>,
      pub in_catalog: bool,        // Already downloaded
      pub in_queue: bool,          // Currently queued
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct SearchResults {
      pub results: Vec<SearchResult>,
      pub total: usize,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct DiscographyResult {
      pub artist: SearchResult,
      pub albums: Vec<SearchResult>,
  }

  #[derive(Debug, Clone, Deserialize)]
  pub struct AlbumRequest {
      pub album_id: String,
      pub album_name: String,
      pub artist_name: String,
  }

  #[derive(Debug, Clone, Deserialize)]
  pub struct DiscographyRequest {
      pub artist_id: String,
      pub artist_name: String,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct RequestResult {
      pub request_id: String,
      pub status: QueueStatus,
      pub queue_position: usize,
  }

  #[derive(Debug, Clone, Serialize)]
  pub struct DiscographyRequestResult {
      pub request_ids: Vec<String>,
      pub albums_queued: usize,
      pub albums_skipped: usize,  // Already in catalog
      pub status: QueueStatus,
  }
  ```

- [x] **Task DM-1.2.7: Define watchdog report type**

  **File:** `catalog-server/src/download_manager/models.rs`

  ```rust
  #[derive(Debug, Default, Clone)]
  pub struct WatchdogReport {
      pub missing_track_audio: Vec<String>,     // Track IDs (base62)
      pub missing_album_images: Vec<String>,    // Image IDs (hex) from album covers
      pub missing_artist_images: Vec<String>,   // Image IDs (hex) from artist portraits
      pub items_queued: usize,
      pub items_skipped: usize,  // Already in queue
      pub scan_duration_ms: i64,
  }
  ```

- [x] **Task DM-1.2.8: Add unit tests for model serialization**

  **File:** `catalog-server/src/download_manager/models.rs` (test module)

  **Test cases:**
  - Enum serialization matches expected strings
  - QueueItem to JSON and back
  - AuditLogEntry with various detail payloads

#### DM-1.3 Database Schema

- [x] **Task DM-1.3.1: Create `download_queue.db` schema**

  **File:** `catalog-server/src/download_manager/schema.rs`

  ```rust
  use crate::sqlite_persistence::VersionedSchema;

  pub const DOWNLOAD_QUEUE_VERSIONED_SCHEMAS: &[VersionedSchema] = &[
      VersionedSchema {
          version: 1,
          up: r#"
              -- Main queue table
              CREATE TABLE download_queue (
                  id TEXT PRIMARY KEY,
                  parent_id TEXT,                      -- Parent queue item ID (for child items)
                  status TEXT NOT NULL,
                  priority INTEGER NOT NULL,
                  content_type TEXT NOT NULL,
                  content_id TEXT NOT NULL,
                  content_name TEXT,
                  artist_name TEXT,
                  request_source TEXT NOT NULL,
                  requested_by_user_id TEXT,
                  created_at INTEGER NOT NULL,
                  started_at INTEGER,
                  completed_at INTEGER,
                  last_attempt_at INTEGER,
                  next_retry_at INTEGER,
                  retry_count INTEGER DEFAULT 0,
                  max_retries INTEGER DEFAULT 5,
                  error_type TEXT,
                  error_message TEXT,
                  bytes_downloaded INTEGER,
                  processing_duration_ms INTEGER,
                  FOREIGN KEY (parent_id) REFERENCES download_queue(id) ON DELETE CASCADE
              );

              CREATE INDEX idx_queue_status_priority ON download_queue(status, priority, created_at);
              CREATE INDEX idx_queue_content ON download_queue(content_type, content_id);
              CREATE INDEX idx_queue_user ON download_queue(requested_by_user_id);
              CREATE INDEX idx_queue_parent ON download_queue(parent_id) WHERE parent_id IS NOT NULL;
              CREATE INDEX idx_queue_next_retry ON download_queue(next_retry_at) WHERE status = 'RETRY_WAITING';

              -- Activity tracking for capacity limits
              CREATE TABLE download_activity_log (
                  hour_bucket INTEGER PRIMARY KEY,
                  albums_downloaded INTEGER DEFAULT 0,
                  tracks_downloaded INTEGER DEFAULT 0,
                  images_downloaded INTEGER DEFAULT 0,
                  bytes_downloaded INTEGER DEFAULT 0,
                  failed_count INTEGER DEFAULT 0,
                  last_updated_at INTEGER NOT NULL
              );

              -- Per-user rate limiting
              CREATE TABLE user_request_stats (
                  user_id TEXT PRIMARY KEY,
                  requests_today INTEGER DEFAULT 0,
                  requests_in_queue INTEGER DEFAULT 0,
                  last_request_date TEXT,
                  last_updated_at INTEGER NOT NULL
              );

              -- Audit log
              CREATE TABLE download_audit_log (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  timestamp INTEGER NOT NULL,
                  event_type TEXT NOT NULL,
                  queue_item_id TEXT,
                  content_type TEXT,
                  content_id TEXT,
                  user_id TEXT,
                  request_source TEXT,
                  details TEXT
              );

              CREATE INDEX idx_audit_timestamp ON download_audit_log(timestamp);
              CREATE INDEX idx_audit_queue_item ON download_audit_log(queue_item_id);
              CREATE INDEX idx_audit_user ON download_audit_log(user_id);
              CREATE INDEX idx_audit_event_type ON download_audit_log(event_type);
              CREATE INDEX idx_audit_content ON download_audit_log(content_type, content_id);
          "#,
      },
  ];
  ```

- [x] **Task DM-1.3.2: Implement schema initialization in store**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Context:** Use existing `VersionedSchema` system from `sqlite_persistence/`.

#### DM-1.4 Queue Store

- [x] **Task DM-1.4.1: Define `DownloadQueueStore` trait**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  ```rust
  use anyhow::Result;
  use super::models::*;

  pub trait DownloadQueueStore: Send + Sync {
      // === Queue Management ===
      fn enqueue(&self, item: QueueItem) -> Result<()>;
      fn get_item(&self, id: &str) -> Result<Option<QueueItem>>;
      fn get_next_pending(&self) -> Result<Option<QueueItem>>;  // By priority, then age
      fn list_by_user(&self, user_id: &str, status: Option<QueueStatus>, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
      fn list_all(&self, status: Option<QueueStatus>, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
      fn get_queue_position(&self, id: &str) -> Result<Option<usize>>;

      // === State Transitions (atomic) ===
      fn claim_for_processing(&self, id: &str) -> Result<bool>;  // PENDING → IN_PROGRESS
      fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;
      fn mark_retry_waiting(&self, id: &str, next_retry_at: i64, error: &DownloadError) -> Result<()>;
      fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()>;

      // === Parent-Child Management ===
      fn create_children(&self, parent_id: &str, children: Vec<QueueItem>) -> Result<()>;
      fn get_children(&self, parent_id: &str) -> Result<Vec<QueueItem>>;
      fn get_children_progress(&self, parent_id: &str) -> Result<DownloadProgress>;
      fn check_parent_completion(&self, parent_id: &str) -> Result<Option<QueueStatus>>;  // Returns new status if all children terminal
      fn get_user_requests(&self, user_id: &str, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;  // parent_id IS NULL only

      // === Retry Handling ===
      fn get_retry_ready(&self) -> Result<Vec<QueueItem>>;  // next_retry_at <= now
      fn promote_retry_to_pending(&self, id: &str) -> Result<()>;

      // === Duplicate/Existence Checks ===
      fn find_by_content(&self, content_type: DownloadContentType, content_id: &str) -> Result<Option<QueueItem>>;
      fn is_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool>;
      fn is_in_active_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool>;  // Non-terminal status

      // === User Rate Limiting ===
      fn get_user_stats(&self, user_id: &str) -> Result<UserLimitStatus>;
      fn increment_user_requests(&self, user_id: &str) -> Result<()>;
      fn decrement_user_queue(&self, user_id: &str) -> Result<()>;
      fn reset_daily_user_stats(&self) -> Result<usize>;  // Reset all users, return count

      // === Activity Tracking ===
      fn record_activity(&self, content_type: DownloadContentType, bytes: u64, success: bool) -> Result<()>;
      fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>>;
      fn get_hourly_counts(&self) -> Result<HourlyCounts>;
      fn get_daily_counts(&self) -> Result<DailyCounts>;

      // === Statistics ===
      fn get_queue_stats(&self) -> Result<QueueStats>;
      fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
      fn get_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<Vec<QueueItem>>;  // For alerting, not auto-cleanup

      // === Audit Logging ===
      fn log_audit_event(&self, event: AuditLogEntry) -> Result<()>;
      fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;  // (entries, total_count)
      fn get_audit_for_item(&self, queue_item_id: &str) -> Result<Vec<AuditLogEntry>>;
      fn get_audit_for_user(&self, user_id: &str, since: Option<i64>, until: Option<i64>, limit: usize, offset: usize) -> Result<(Vec<AuditLogEntry>, usize)>;
      fn cleanup_old_audit_entries(&self, older_than: i64) -> Result<usize>;
  }
  ```

- [x] **Task DM-1.4.2: Implement `SqliteDownloadQueueStore` constructor and schema init**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  ```rust
  pub struct SqliteDownloadQueueStore {
      conn: Arc<Mutex<Connection>>,
  }

  impl SqliteDownloadQueueStore {
      pub fn new(db_path: &Path) -> Result<Self> {
          let conn = Connection::open(db_path)?;
          // Apply schema migrations
          apply_versioned_schema(&conn, DOWNLOAD_QUEUE_VERSIONED_SCHEMAS)?;
          Ok(Self {
              conn: Arc::new(Mutex::new(conn)),
          })
      }
  }
  ```

- [x] **Task DM-1.4.3: Implement queue management methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `enqueue`, `get_item`, `get_next_pending`, `list_by_user`, `list_all`, `get_queue_position`

- [x] **Task DM-1.4.4: Implement state transition methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `claim_for_processing`, `mark_completed`, `mark_retry_waiting`, `mark_failed`

  **Important:** Use transactions for atomic state changes. Verify current status before transition.

- [x] **Task DM-1.4.5: Implement parent-child management methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `create_children`, `get_children`, `get_children_progress`, `check_parent_completion`, `get_user_requests`

  **Logic for `check_parent_completion`:**
  - Query children: `SELECT status FROM download_queue WHERE parent_id = ?`
  - If any child still non-terminal (PENDING, IN_PROGRESS, RETRY_WAITING) → return None
  - If all children COMPLETED → return Some(COMPLETED)
  - If any child FAILED (and all others terminal) → return Some(FAILED)

- [x] **Task DM-1.4.6: Implement retry handling methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `get_retry_ready`, `promote_retry_to_pending`

- [x] **Task DM-1.4.7: Implement duplicate check methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `find_by_content`, `is_in_queue`, `is_in_active_queue`

- [x] **Task DM-1.4.8: Implement user rate limiting methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `get_user_stats`, `increment_user_requests`, `decrement_user_queue`, `reset_daily_user_stats`

  **Note:** Daily reset should check `last_request_date` and reset if different from today.

- [x] **Task DM-1.4.9: Implement activity tracking methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `record_activity`, `get_activity_since`, `get_hourly_counts`, `get_daily_counts`

  **Note:** Use hour-truncated timestamps as bucket keys.

- [x] **Task DM-1.4.10: Implement statistics methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `get_queue_stats`, `get_failed_items`, `get_stale_in_progress`

- [x] **Task DM-1.4.11: Implement audit logging methods**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

  **Methods:** `log_audit_event`, `get_audit_log`, `get_audit_for_item`, `get_audit_for_user`, `cleanup_old_audit_entries`

- [x] **Task DM-1.4.12: Add unit tests for queue store**

  **File:** `catalog-server/src/download_manager/queue_store.rs` (test module)

  **Test cases:** (103 tests implemented covering all scenarios)
  - Enqueue and retrieve items
  - Priority ordering (watchdog > user > expansion)
  - State transitions (atomic, valid transitions only)
  - Parent-child: create children, get progress, check completion
  - Parent completion: all children complete → parent complete
  - Parent failure: any child failed → parent failed
  - User stats tracking and daily reset
  - Activity logging and hourly/daily counts
  - Duplicate detection
  - Audit log CRUD and filtering

#### DM-1.5 Support Components

- [x] **Task DM-1.5.1: Implement `RetryPolicy`**

  **File:** `catalog-server/src/download_manager/retry_policy.rs`

  ```rust
  #[derive(Debug, Clone)]
  pub struct RetryPolicy {
      pub max_retries: i32,
      pub initial_backoff_secs: u64,
      pub max_backoff_secs: u64,
      pub backoff_multiplier: f64,
  }

  impl RetryPolicy {
      pub fn new(config: &DownloadManagerSettings) -> Self {
          Self {
              max_retries: config.max_retries as i32,
              initial_backoff_secs: config.initial_backoff_secs,
              max_backoff_secs: config.max_backoff_secs,
              backoff_multiplier: config.backoff_multiplier,
          }
      }

      /// Calculate next retry time based on current retry count
      pub fn next_retry_at(&self, retry_count: i32) -> i64 {
          let backoff = self.initial_backoff_secs as f64
              * self.backoff_multiplier.powi(retry_count);
          let capped_backoff = backoff.min(self.max_backoff_secs as f64) as i64;
          chrono::Utc::now().timestamp() + capped_backoff
      }

      /// Check if error should be retried
      pub fn should_retry(&self, error: &DownloadError, retry_count: i32) -> bool {
          error.is_retryable() && retry_count < self.max_retries
      }
  }
  ```

- [x] **Task DM-1.5.2: Add unit tests for `RetryPolicy`**

  **File:** `catalog-server/src/download_manager/retry_policy.rs` (test module)

  **Test cases:**
  - Backoff calculation at each retry level
  - Max backoff capping
  - Not-found errors are not retried
  - Other errors are retried up to max

- [x] **Task DM-1.5.3: Implement `AuditLogger` helper**

  **File:** `catalog-server/src/download_manager/audit_logger.rs`

  ```rust
  pub struct AuditLogger {
      queue_store: Arc<dyn DownloadQueueStore>,
  }

  impl AuditLogger {
      pub fn new(queue_store: Arc<dyn DownloadQueueStore>) -> Self {
          Self { queue_store }
      }

      pub fn log_request_created(
          &self,
          queue_item: &QueueItem,
          queue_position: usize,
      ) -> Result<()>;

      pub fn log_download_started(&self, queue_item: &QueueItem) -> Result<()>;

      pub fn log_children_created(
          &self,
          parent_item: &QueueItem,
          children_count: usize,
          track_count: usize,
          image_count: usize,
      ) -> Result<()>;

      pub fn log_download_completed(
          &self,
          queue_item: &QueueItem,
          bytes_downloaded: u64,
          duration_ms: i64,
          tracks_downloaded: Option<usize>,
      ) -> Result<()>;

      pub fn log_download_failed(
          &self,
          queue_item: &QueueItem,
          error: &DownloadError,
      ) -> Result<()>;

      pub fn log_retry_scheduled(
          &self,
          queue_item: &QueueItem,
          next_retry_at: i64,
          backoff_secs: u64,
          error: &DownloadError,
      ) -> Result<()>;

      pub fn log_admin_retry(
          &self,
          queue_item: &QueueItem,
          admin_user_id: &str,
      ) -> Result<()>;

      pub fn log_watchdog_queued(
          &self,
          queue_item: &QueueItem,
          reason: &str,
      ) -> Result<()>;

      pub fn log_watchdog_scan_started(&self) -> Result<()>;

      pub fn log_watchdog_scan_completed(&self, report: &WatchdogReport) -> Result<()>;
  }
  ```

- [x] **Task DM-1.5.4: Add unit tests for `AuditLogger`**

  **File:** `catalog-server/src/download_manager/audit_logger.rs` (test module)

#### DM-1.6 Permission

- [x] **Task DM-1.6.1: Add `RequestContent` permission (ID = 9)**

  **File:** `catalog-server/src/user/permissions.rs`

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Permission {
      // ... existing permissions ...
      RequestContent,  // ID = 9
  }

  impl Permission {
      pub fn to_int(&self) -> i32 {
          match self {
              // ... existing ...
              Permission::RequestContent => 9,
          }
      }

      pub fn from_int(i: i32) -> Option<Permission> {
          match i {
              // ... existing ...
              9 => Some(Permission::RequestContent),
              _ => None,
          }
      }
  }
  ```

- [x] **Task DM-1.6.2: Update Admin role to include RequestContent**

  **File:** `catalog-server/src/user/permissions.rs`

  **Update `ADMIN_PERMISSIONS` array to include `RequestContent`.**

- [x] **Task DM-1.6.3: Add `require_request_content` middleware**

  **File:** `catalog-server/src/server/server.rs`

  ```rust
  fn require_request_content<B>(
      State(state): State<ServerState>,
      request: Request<B>,
      next: Next<B>,
  ) -> impl Future<Output = Response> {
      require_permission(Permission::RequestContent, state, request, next)
  }
  ```

- [x] **Task DM-1.6.4: Update permission documentation**

  **File:** `catalog-server/README.md`

  **Add RequestContent to permissions table with description:**
  > Allows searching external music provider and requesting content downloads.

- [x] **Task DM-1.6.5: Update web frontend permission references**

  **Files:**
  - `web/src/store/user.js`: Add `'RequestContent'` to known permissions
  - Add `canRequestContent` computed property

- [x] **Task DM-1.6.6: Update Android permission references**

  **Files:**
  - `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/model/Permission.kt`: Add `RequestContent`
  - `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`: Add mapping

#### DM-1.7 DownloadManager Struct

- [x] **Task DM-1.7.1: Create `DownloadManager` struct**

  **File:** `catalog-server/src/download_manager/manager.rs`

  ```rust
  pub struct DownloadManager {
      queue_store: Arc<dyn DownloadQueueStore>,
      downloader_client: DownloaderClient,
      catalog_store: Arc<dyn CatalogStore>,
      media_path: PathBuf,
      config: DownloadManagerSettings,
      retry_policy: RetryPolicy,
      audit_logger: AuditLogger,
  }

  impl DownloadManager {
      pub fn new(
          queue_store: Arc<dyn DownloadQueueStore>,
          downloader_client: DownloaderClient,
          catalog_store: Arc<dyn CatalogStore>,
          media_path: PathBuf,
          config: DownloadManagerSettings,
      ) -> Self;

      // Search proxy (async - calls external downloader service)
      pub async fn search(&self, query: &str, search_type: SearchType) -> Result<SearchResults>;
      pub async fn search_discography(&self, artist_id: &str) -> Result<DiscographyResult>;

      // User requests (sync - only touches local queue store)
      pub fn request_album(&self, user_id: &str, request: AlbumRequest) -> Result<RequestResult>;
      pub fn request_discography(&self, user_id: &str, request: DiscographyRequest) -> Result<DiscographyRequestResult>;
      pub fn get_user_requests(&self, user_id: &str, status: Option<QueueStatus>) -> Result<Vec<UserRequestView>>;
      pub fn get_request_status(&self, user_id: &str, request_id: &str) -> Result<UserRequestView>;

      // Rate limiting (sync - only touches local queue store)
      pub fn check_user_limits(&self, user_id: &str) -> Result<UserLimitStatus>;
      pub fn check_global_capacity(&self) -> Result<CapacityStatus>;

      // Queue processing (async - calls external downloader service)
      pub async fn process_next(&self) -> Result<Option<ProcessingResult>>;
      pub fn promote_ready_retries(&self) -> Result<usize>;

      // Admin (sync - only touches local queue store)
      pub fn get_queue_stats(&self) -> Result<QueueStats>;
      pub fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
      pub fn retry_failed(&self, admin_user_id: &str, request_id: &str) -> Result<()>;
      pub fn get_activity(&self, hours: usize) -> Result<Vec<ActivityLogEntry>>;
      pub fn get_all_requests(&self, status: Option<QueueStatus>, user_id: Option<&str>, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;

      // Audit (sync - only touches local queue store)
      pub fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;
      pub fn get_audit_for_item(&self, queue_item_id: &str) -> Result<(QueueItem, Vec<AuditLogEntry>)>;
      pub fn get_audit_for_user(&self, user_id: &str, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;
  }
  ```

- [x] **Task DM-1.7.2: Implement search proxy methods in DownloadManager**

  **File:** `catalog-server/src/download_manager/manager.rs`

  **Context:** Delegates to `search_proxy.rs` module.

- [x] **Task DM-1.7.3: Implement user request methods in DownloadManager**

  **File:** `catalog-server/src/download_manager/manager.rs`

  **Logic:**
  1. Check user rate limits
  2. Check if already in catalog
  3. Check if already in queue
  4. Enqueue with appropriate priority
  5. Log audit event
  6. Return result with queue position

- [x] **Task DM-1.7.4: Implement queue processing in DownloadManager**

  **File:** `catalog-server/src/download_manager/manager.rs`

  **Logic for `process_next()`:**
  1. Get next pending item (by priority)
  2. Claim for processing (atomic)
  3. Log download started
  4. Call downloader
  5. On success: mark completed, record activity, log audit
  6. On failure: check retry policy, either mark retry or failed, log audit

- [x] **Task DM-1.7.5: Add unit tests for DownloadManager**

  **File:** `catalog-server/src/download_manager/manager.rs` (test module)

---

### Phase DM-2: Search Proxy

#### DM-2.1 HTTP Client

- [x] **Task DM-2.1.1: Create `DownloaderClient` for HTTP communication**

  **File:** `catalog-server/src/download_manager/downloader_client.rs`

  ```rust
  pub struct DownloaderClient {
      client: reqwest::Client,
      base_url: String,
      timeout: Duration,
  }

  impl DownloaderClient {
      pub fn new(base_url: &str, timeout_secs: u64) -> Self {
          let client = reqwest::Client::builder()
              .timeout(Duration::from_secs(timeout_secs))
              .build()
              .expect("Failed to create HTTP client");

          Self {
              client,
              base_url: base_url.to_string(),
              timeout: Duration::from_secs(timeout_secs),
          }
      }

      // Search endpoints
      pub async fn search(&self, query: &str, search_type: SearchType) -> Result<Vec<ExternalSearchResult>>;
      pub async fn get_discography(&self, artist_id: &str) -> Result<ExternalDiscographyResult>;

      // Metadata endpoints
      pub async fn get_album(&self, album_id: &str) -> Result<ExternalAlbum>;
      pub async fn get_album_tracks(&self, album_id: &str) -> Result<Vec<ExternalTrack>>;
      pub async fn get_artist(&self, artist_id: &str) -> Result<ExternalArtist>;
      pub async fn get_track(&self, track_id: &str) -> Result<ExternalTrack>;

      // Download endpoints (return raw bytes)
      pub async fn download_track_audio(&self, track_id: &str) -> Result<(Vec<u8>, String)>;  // (bytes, content_type)
      pub async fn download_image(&self, image_id: &str) -> Result<Vec<u8>>;
  }
  ```

- [x] **Task DM-2.1.2: Define external API response types**

  **File:** `catalog-server/src/download_manager/downloader_types.rs`

  **Search types:**
  ```rust
  #[derive(Debug, Deserialize)]
  pub struct ExternalSearchResult {
      pub id: String,
      #[serde(rename = "type")]
      pub result_type: String,
      pub name: String,
      pub artist_name: Option<String>,
      pub image_url: Option<String>,
      pub year: Option<i32>,
  }

  #[derive(Debug, Deserialize)]
  pub struct ExternalDiscographyResult {
      pub artist: ExternalSearchResult,
      pub albums: Vec<ExternalSearchResult>,
  }
  ```

  **Metadata types (matching downloader API responses):**
  ```rust
  #[derive(Debug, Deserialize)]
  pub struct ExternalAlbum {
      pub id: String,
      pub name: String,
      pub album_type: String,
      pub artists_ids: Vec<String>,
      pub label: String,
      pub date: i64,  // Unix timestamp
      pub genres: Vec<String>,
      pub covers: Vec<ExternalImage>,
      pub discs: Vec<ExternalDisc>,
  }

  #[derive(Debug, Deserialize)]
  pub struct ExternalDisc {
      pub number: i32,
      pub name: String,
      pub tracks: Vec<String>,  // Track IDs
  }

  #[derive(Debug, Deserialize)]
  pub struct ExternalImage {
      pub id: String,  // Hex ID for download
      pub size: String,
      pub width: i32,
      pub height: i32,
  }

  #[derive(Debug, Deserialize)]
  pub struct ExternalTrack {
      pub id: String,
      pub name: String,
      pub album_id: String,
      pub artists_ids: Vec<String>,
      pub number: i32,
      pub disc_number: i32,
      pub duration: i64,  // milliseconds
      pub is_explicit: bool,
  }

  #[derive(Debug, Deserialize)]
  pub struct ExternalArtist {
      pub id: String,
      pub name: String,
      pub genre: Vec<String>,
      pub portraits: Vec<ExternalImage>,
  }
  ```

#### DM-2.2 Search Implementation

- [x] **Task DM-2.2.1: Implement search with enrichment**

  **File:** `catalog-server/src/download_manager/search_proxy.rs`

  **Logic:**
  1. Call downloader service search endpoint
  2. For each result, check `in_catalog` via catalog store
  3. For each result, check `in_queue` via queue store
  4. Return enriched results

- [x] **Task DM-2.2.2: Implement discography search with enrichment**

  **File:** `catalog-server/src/download_manager/search_proxy.rs`

- [ ] **Task DM-2.2.3: Add caching for catalog/queue checks (optional)**

  **Context:** If search results are large, batch the existence checks.

#### DM-2.3 API Handlers

- [x] **Task DM-2.3.1: Create `GET /v1/download/search` handler**

  **File:** `catalog-server/src/server/server.rs`

  ```rust
  async fn search_download_content(
      State(state): State<ServerState>,
      session: Session,
      Query(params): Query<SearchParams>,
  ) -> Result<Json<SearchResults>, ApiError> {
      require_permission(&session, Permission::RequestContent)?;

      let dm = state.download_manager.as_ref()
          .ok_or(ApiError::ServiceUnavailable("Download manager not enabled"))?;

      let search_type = match params.content_type.as_deref() {
          Some("artist") => SearchType::Artist,
          _ => SearchType::Album,
      };

      dm.search(&params.q, search_type).await.map(Json)
  }
  ```

- [x] **Task DM-2.3.2: Create `GET /v1/download/search/discography/:artist_id` handler**

  **File:** `catalog-server/src/server/server.rs`

---

### Phase DM-3: User Request API

#### DM-3.1 Request Handlers

- [x] **Task DM-3.1.1: Create `POST /v1/download/request/album` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Response:**
  ```json
  {
      "request_id": "uuid",
      "status": "PENDING",
      "queue_position": 5
  }
  ```

  **Errors:**
  - 429: Rate limit exceeded (daily or queue)
  - 409: Already in catalog or queue
  - 503: Download manager not enabled

- [x] **Task DM-3.1.2: Create `POST /v1/download/request/discography` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Response:**
  ```json
  {
      "request_ids": ["uuid1", "uuid2"],
      "albums_queued": 5,
      "albums_skipped": 3,
      "status": "PENDING"
  }
  ```

- [x] **Task DM-3.1.3: Create `GET /v1/download/my-requests` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Response:**
  ```json
  {
      "requests": [UserRequestView],
      "stats": {
          "requests_today": 15,
          "max_per_day": 100,
          "in_queue": 8,
          "max_queue": 200
      }
  }
  ```

- [x] **Task DM-3.1.4: Create `GET /v1/download/request/:id` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Note:** Only returns requests owned by the current user.

- [x] **Task DM-3.1.5: Create `GET /v1/download/limits` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `RequestContent`

  **Response:**
  ```json
  {
      "user": {
          "requests_today": 15,
          "max_per_day": 100,
          "in_queue": 8,
          "max_queue": 200,
          "can_request": true
      },
      "global": {
          "albums_this_hour": 7,
          "max_per_hour": 10,
          "albums_today": 52,
          "max_per_day": 60,
          "at_capacity": false
      }
  }
  ```

  **Note:** Allows users to check their limits before attempting a request.

#### DM-3.2 Rate Limiting

- [x] **Task DM-3.2.1: Implement rate limit checking in request handlers**

  **File:** `catalog-server/src/server/server.rs`

  **Logic:**
  1. Check `user_request_stats.requests_today < config.user_max_requests_per_day`
  2. Check `user_request_stats.requests_in_queue < config.user_max_queue_size`
  3. Return 429 with helpful error message if exceeded

- [x] **Task DM-3.2.2: Create daily stats reset background job** *(Deferred - automatic reset on read)*

  **File:** `catalog-server/src/background_jobs/jobs/reset_download_stats.rs`

  **Context:** Runs daily at midnight to reset `requests_today` for all users.

  **Note:** The `get_user_stats` method already handles daily reset automatically when the date changes. The explicit background job is optional for database cleanup and deferred until JobContext is extended to include the download queue store.

---

### Phase DM-4: Queue Processor

#### DM-4.1 Job Processor

- [x] **Task DM-4.1.1: Create `QueueProcessor` struct**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

  ```rust
  pub struct QueueProcessor {
      download_manager: Arc<DownloadManager>,
      interval: Duration,
  }

  impl QueueProcessor {
      pub fn new(download_manager: Arc<DownloadManager>, interval_secs: u64) -> Self;

      /// Main processing loop - call from spawned task
      pub async fn run(&self, shutdown: CancellationToken) {
          let mut interval = tokio::time::interval(self.interval);

          loop {
              tokio::select! {
                  _ = interval.tick() => {
                      // Promote ready retries
                      if let Err(e) = self.download_manager.promote_ready_retries() {
                          error!("Failed to promote retries: {}", e);
                      }

                      // Process next item
                      match self.download_manager.process_next().await {
                          Ok(Some(result)) => {
                              info!("Processed download: {:?}", result);
                          }
                          Ok(None) => {
                              // Queue empty
                          }
                          Err(e) => {
                              error!("Queue processor error: {}", e);
                          }
                      }
                  }
                  _ = shutdown.cancelled() => {
                      info!("Queue processor shutting down");
                      break;
                  }
              }
          }
      }
  }
  ```

- [x] **Task DM-4.1.2: Implement download execution logic**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

  **Logic in `process_next()`:**
  1. Check global capacity limits
  2. Get next pending item (priority order, then age)
  3. Claim for processing
  4. Branch based on content type (see DM-4.1.3)
  5. Handle success/failure with retry policy
  6. Record activity metrics
  7. Log audit events
  8. **For child items:** After completion, call `check_parent_completion()` and update parent if all children done

- [x] **Task DM-4.1.3: Handle different content types**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

  **Content type handling (parent-child model):**

  **`Album` (parent item):**
  1. Fetch metadata: `GET /album/:id`, `GET /album/:id/tracks`, `GET /artist/:id` for each artist
  2. Create catalog entries via CatalogStore (see DM-4.1.4)
  3. Create child queue items:
     - `TrackAudio` for each track (`content_id` = track's base62 ID)
     - `AlbumImage` for covers (`content_id` = image's 40-char hex ID from `covers[].id`)
     - `ArtistImage` for portraits (`content_id` = image's 40-char hex ID from `portraits[].id`)
  4. Item stays `IN_PROGRESS` until all children complete (not "downloading" - waiting for children)

  **`TrackAudio` (child item):**
  1. `GET /track/:id/audio` from downloader → returns `(bytes, content_type)`
  2. Determine file extension from content_type (`audio/ogg` → `.ogg`, `audio/mpeg` → `.mp3`, etc.)
  3. Write bytes to `{media_path}/audio/{track_id}.{ext}`
  4. Mark `COMPLETED`, call `check_parent_completion()`

  **`AlbumImage` / `ArtistImage` (child item):**
  1. `GET /image/:id` from downloader (id is 40-char hex)
  2. Write bytes to `{media_path}/images/{id}.jpg`
  3. Mark `COMPLETED`, call `check_parent_completion()`

- [x] **Task DM-4.1.4: Implement catalog ingestion logic**

  **File:** `catalog-server/src/download_manager/catalog_ingestion.rs`

  **Purpose:** Convert external types to catalog types and insert into CatalogStore.

  **Types:**
  ```rust
  /// Result of ingesting an album - contains IDs needed for creating child queue items
  pub struct IngestedAlbum {
      pub album_id: String,
      pub track_ids: Vec<String>,           // Base62 track IDs
      pub album_image_ids: Vec<String>,     // Hex image IDs from covers
      pub artist_image_ids: Vec<String>,    // Hex image IDs from portraits
  }
  ```

  **Logic for Album processing:**
  ```rust
  pub fn ingest_album(
      catalog_store: &dyn CatalogStore,
      album: ExternalAlbum,
      tracks: Vec<ExternalTrack>,
      artists: Vec<ExternalArtist>,
  ) -> Result<IngestedAlbum> {
      // Extract IDs needed for child queue items BEFORE consuming the structs
      let album_id = album.id.clone();
      let album_image_ids: Vec<String> = album.covers.iter().map(|c| c.id.clone()).collect();
      let track_ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
      let artist_image_ids: Vec<String> = artists.iter()
          .flat_map(|a| a.portraits.iter().map(|p| p.id.clone()))
          .collect();

      // 1. For each artist: check if exists, insert if not
      for artist in artists {
          if !catalog_store.artist_exists(&artist.id)? {
              let catalog_artist = convert_artist(artist);
              catalog_store.insert_artist(catalog_artist)?;
          }
      }

      // 2. Insert album (links to artists)
      let catalog_album = convert_album(album);
      catalog_store.insert_album(catalog_album)?;

      // 3. Insert tracks (links to album and artists)
      for track in tracks {
          let catalog_track = convert_track(track);
          catalog_store.insert_track(catalog_track)?;
      }

      // 4. Return info needed for child creation
      Ok(IngestedAlbum {
          album_id,
          track_ids,
          album_image_ids,
          artist_image_ids,
      })
  }
  ```

  **Note:** Uses existing CatalogStore write methods. Verify these methods exist and support the required operations.

#### DM-4.2 Main Integration

- [x] **Task DM-4.2.1: Initialize DownloadManager in main.rs**

  **File:** `catalog-server/src/main.rs`

  ```rust
  let download_manager = if config.download_manager.enabled {
      let queue_store = Arc::new(SqliteDownloadQueueStore::new(&config.download_queue_db_path())?);
      let downloader_client = DownloaderClient::new(
          config.downloader_url.as_ref().unwrap().clone(),
          config.downloader_timeout_sec,
      );

      let manager = Arc::new(DownloadManager::new(
          queue_store,
          downloader_client,
          catalog_store.clone(),
          config.media_path.clone(),
          config.download_manager.clone(),
      ));

      info!("Download manager initialized");
      Some(manager)
  } else {
      info!("Download manager disabled (no downloader_url configured)");
      None
  };
  ```

- [x] **Task DM-4.2.2: Spawn queue processor task**

  **File:** `catalog-server/src/main.rs`

  ```rust
  if let Some(ref dm) = download_manager {
      let processor = QueueProcessor::new(
          dm.clone(),
          config.download_manager.process_interval_secs,
      );
      let shutdown = shutdown_token.child_token();
      tokio::spawn(async move {
          processor.run(shutdown).await;
      });
  }
  ```

- [x] **Task DM-4.2.3: Add DownloadManager to ServerState**

  **File:** `catalog-server/src/server/server.rs`

  ```rust
  pub struct ServerState {
      // ... existing fields ...
      pub download_manager: Option<Arc<DownloadManager>>,
  }
  ```

#### DM-4.3 Stale Detection

- [ ] **Task DM-4.3.1: Implement stale in-progress detection and alerting**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

  **Logic:** Items stuck in IN_PROGRESS longer than `stale_in_progress_threshold_secs` should trigger alerts (warning logs + Prometheus metric increment). Do NOT auto-fail - stale items indicate something is broken and needs human investigation.

  **Trigger:** Run on startup and periodically (hourly).

  **Metric:** `pezzottify_download_stale_in_progress` gauge showing count of stale items.

---

### Phase DM-5: Integrity Watchdog

#### DM-5.1 Watchdog Implementation

- [ ] **Task DM-5.1.1: Create `IntegrityWatchdog` struct**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

  ```rust
  pub struct IntegrityWatchdog {
      catalog_store: Arc<dyn CatalogStore>,
      queue_store: Arc<dyn DownloadQueueStore>,
      audit_logger: AuditLogger,
      media_path: PathBuf,
  }

  impl IntegrityWatchdog {
      pub fn new(
          catalog_store: Arc<dyn CatalogStore>,
          queue_store: Arc<dyn DownloadQueueStore>,
          audit_logger: AuditLogger,
          media_path: PathBuf,
      ) -> Self;

      /// Run full integrity scan
      pub fn run_scan(&self) -> Result<WatchdogReport>;
  }
  ```

- [ ] **Task DM-5.1.2: Implement `scan_missing_track_audio`**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

  **Logic:**
  1. Query all tracks from catalog
  2. For each track, check if audio file exists at expected path
  3. Return list of track IDs with missing audio

- [ ] **Task DM-5.1.3: Implement `scan_missing_album_images`**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

- [ ] **Task DM-5.1.4: Implement `scan_missing_artist_images`**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

- [ ] **Task DM-5.1.5: Implement `queue_repairs`**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

  **Logic:**
  1. For each missing item, check if already in queue
  2. Skip if in queue
  3. Enqueue with priority = Watchdog (1)
  4. Log audit event for each queued item
  5. Return count of queued vs skipped

#### DM-5.2 Background Job

- [ ] **Task DM-5.2.1: Create `IntegrityWatchdogJob`**

  **File:** `catalog-server/src/background_jobs/jobs/integrity_watchdog.rs`

  ```rust
  pub struct IntegrityWatchdogJob {
      watchdog: Arc<IntegrityWatchdog>,
  }

  impl BackgroundJob for IntegrityWatchdogJob {
      fn id(&self) -> &'static str { "integrity_watchdog" }
      fn name(&self) -> &'static str { "Integrity Watchdog" }
      fn description(&self) -> &'static str { "Scan catalog for missing files and queue repairs" }

      fn schedule(&self) -> JobSchedule {
          JobSchedule::Combined {
              interval: Some(Duration::from_secs(24 * 60 * 60)),  // Daily
              cron: None,
              hooks: vec![HookEvent::OnStartup],  // Also run on startup
          }
      }

      fn shutdown_behavior(&self) -> ShutdownBehavior {
          ShutdownBehavior::Cancellable
      }

      fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
          let report = self.watchdog.run_scan()
              .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

          info!("Watchdog scan complete: queued={}, skipped={}, duration={}ms",
              report.items_queued, report.items_skipped, report.scan_duration_ms);

          Ok(())
      }
  }
  ```

- [ ] **Task DM-5.2.2: Register watchdog job conditionally**

  **File:** `catalog-server/src/main.rs`

  ```rust
  if let Some(ref dm) = download_manager {
      let watchdog = Arc::new(IntegrityWatchdog::new(
          catalog_store.clone(),
          dm.queue_store.clone(),
          dm.audit_logger.clone(),
          config.media_path.clone(),
      ));

      scheduler.register_job(Arc::new(IntegrityWatchdogJob::new(watchdog)));
  }
  ```

- [ ] **Task DM-5.2.3: Add unit tests for watchdog**

  **File:** `catalog-server/src/download_manager/watchdog.rs` (test module)

---

### Phase DM-6: Admin API & Polish

#### DM-6.1 Admin Handlers

- [ ] **Task DM-6.1.1: Create `GET /v1/download/admin/stats` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Response:**
  ```json
  {
      "queue": {
          "pending": 10,
          "in_progress": 2,
          "retry_waiting": 3,
          "completed_today": 45,
          "failed_today": 2
      },
      "capacity": {
          "albums_this_hour": 7,
          "max_per_hour": 10,
          "albums_today": 52,
          "max_per_day": 60
      },
      "processing": {
          "average_duration_ms": 2500,
          "success_rate_percent": 95.5
      }
  }
  ```

- [ ] **Task DM-6.1.2: Create `GET /v1/download/admin/failed` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Query params:** `limit`, `offset`

- [ ] **Task DM-6.1.3: Create `POST /v1/download/admin/retry/:id` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `EditCatalog`

  **Logic:** Reset failed item to pending, log admin retry audit event.

- [ ] **Task DM-6.1.4: Create `GET /v1/download/admin/activity` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Query params:** `hours` (default 24)

  **Response:**
  ```json
  {
      "hourly": [
          { "hour": "2024-01-15T10:00:00Z", "albums": 8, "tracks": 45, "bytes": 512000000 }
      ],
      "totals": {
          "albums": 52,
          "tracks": 312,
          "bytes": 3200000000
      }
  }
  ```

- [ ] **Task DM-6.1.5: Create `GET /v1/download/admin/requests` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Query params:** `status`, `user_id`, `limit`, `offset`

#### DM-6.2 Audit Log Handlers

- [ ] **Task DM-6.2.1: Create `GET /v1/download/admin/audit` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Query params:** `queue_item_id`, `user_id`, `event_type`, `content_type`, `content_id`, `since`, `until`, `limit`, `offset`

  **Response:**
  ```json
  {
      "entries": [AuditLogEntry],
      "total_count": 1234,
      "has_more": true
  }
  ```

- [ ] **Task DM-6.2.2: Create `GET /v1/download/admin/audit/item/:queue_item_id` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

  **Response:**
  ```json
  {
      "queue_item": QueueItem,
      "events": [AuditLogEntry]
  }
  ```

- [ ] **Task DM-6.2.3: Create `GET /v1/download/admin/audit/user/:user_id` handler**

  **File:** `catalog-server/src/server/server.rs`

  **Permission:** `ViewAnalytics`

#### DM-6.3 Cleanup Jobs

- [ ] **Task DM-6.3.1: Create audit log cleanup background job**

  **File:** `catalog-server/src/background_jobs/jobs/audit_log_cleanup.rs`

  **Context:** Uses `config.download_manager.audit_log_retention_days`. Runs daily. Deletes entries older than retention period.

- [ ] **Task DM-6.3.2: Register audit log cleanup job**

  **File:** `catalog-server/src/main.rs`

#### DM-6.4 Router Wiring

- [ ] **Task DM-6.4.1: Create `download_routes` function**

  **File:** `catalog-server/src/server/server.rs`

  ```rust
  fn download_routes(state: ServerState) -> Router {
      Router::new()
          // Search
          .route("/search", get(search_download_content))
          .route("/search/discography/:artist_id", get(search_discography))
          // User requests
          .route("/request/album", post(request_album))
          .route("/request/discography", post(request_discography))
          .route("/my-requests", get(list_my_requests))
          .route("/request/:id", get(get_request_status))
          .route("/limits", get(get_user_limits))
          // Admin
          .route("/admin/stats", get(admin_download_stats))
          .route("/admin/failed", get(admin_failed_downloads))
          .route("/admin/retry/:id", post(admin_retry_download))
          .route("/admin/activity", get(admin_download_activity))
          .route("/admin/requests", get(admin_all_requests))
          // Audit
          .route("/admin/audit", get(admin_audit_log))
          .route("/admin/audit/item/:queue_item_id", get(admin_audit_for_item))
          .route("/admin/audit/user/:user_id", get(admin_audit_for_user))
          .with_state(state)
  }
  ```

- [ ] **Task DM-6.4.2: Wire download routes into main router**

  **File:** `catalog-server/src/server/server.rs`

  ```rust
  let app = Router::new()
      // ... existing routes ...
      .nest("/v1/download", download_routes(state.clone()));
  ```

#### DM-6.5 Tests

- [ ] **Task DM-6.5.1: Write integration tests for user request flow**

  **Test scenario:**
  1. User submits album request
  2. Verify rate limits checked
  3. Verify item queued
  4. Process queue
  5. Verify completed status
  6. Verify audit log entries

- [ ] **Task DM-6.5.2: Write integration tests for rate limiting**

  **Test scenario:**
  1. Submit requests up to daily limit
  2. Verify 429 on next request
  3. Verify queue limit separately

- [ ] **Task DM-6.5.3: Write integration tests for watchdog**

  **Test scenario:**
  1. Create album in catalog without audio files
  2. Run watchdog scan
  3. Verify items queued at priority 1
  4. Verify audit events

- [ ] **Task DM-6.5.4: Write integration tests for admin API**

  **Test scenarios:**
  - Stats endpoint returns correct counts
  - Failed items list
  - Admin retry resets status
  - Audit log queries

---

### Phase DM-7: Metrics

- [ ] **Task DM-7.1: Define Prometheus metrics**

  **File:** `catalog-server/src/server/metrics.rs`

  ```rust
  lazy_static! {
      // Queue size by status
      pub static ref DOWNLOAD_QUEUE_SIZE: IntGaugeVec = IntGaugeVec::new(
          Opts::new("pezzottify_download_queue_size", "Current queue size"),
          &["status", "priority"]
      ).unwrap();

      // Processing metrics
      pub static ref DOWNLOAD_PROCESSED_TOTAL: IntCounterVec = IntCounterVec::new(
          Opts::new("pezzottify_download_processed_total", "Processed downloads"),
          &["content_type", "result"]  // result: completed, failed, retry
      ).unwrap();

      pub static ref DOWNLOAD_PROCESSING_DURATION: Histogram = Histogram::with_opts(
          HistogramOpts::new("pezzottify_download_processing_seconds", "Download processing time")
              .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0])
      ).unwrap();

      // Capacity metrics
      pub static ref DOWNLOAD_CAPACITY_USED: IntGaugeVec = IntGaugeVec::new(
          Opts::new("pezzottify_download_capacity_used", "Capacity usage"),
          &["period"]  // hourly, daily
      ).unwrap();

      // User request metrics
      pub static ref DOWNLOAD_USER_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
          Opts::new("pezzottify_download_user_requests_total", "User download requests"),
          &["type"]  // album, discography
      ).unwrap();

      // Audit log metrics
      pub static ref DOWNLOAD_AUDIT_EVENTS_TOTAL: IntCounterVec = IntCounterVec::new(
          Opts::new("pezzottify_download_audit_events_total", "Audit log events"),
          &["event_type"]
      ).unwrap();
  }
  ```

- [ ] **Task DM-7.2: Register download metrics**

  **File:** `catalog-server/src/server/metrics.rs`

- [ ] **Task DM-7.3: Emit metrics from queue processor**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

- [ ] **Task DM-7.4: Emit metrics from request handlers**

  **File:** `catalog-server/src/server/server.rs`

- [ ] **Task DM-7.5: Update queue size metrics periodically**

  **Context:** Add to queue processor loop to update gauge metrics.

---

### Phase DM-8: Documentation

- [ ] **Task DM-8.1: Update `catalog-server/README.md` with download manager section**

  **Content:**
  - Overview and architecture
  - Configuration options
  - API endpoint documentation
  - Permission requirements

- [ ] **Task DM-8.2: Update `CLAUDE.md` with download manager routes**

  **File:** `CLAUDE.md`

  **Add to server routes structure section.**

- [ ] **Task DM-8.3: Create example TOML config with download manager settings**

  **File:** `catalog-server/config.example.toml`

  **Add download_manager section with all options documented.**

---

## Summary

### Phase Order (Dependencies)

1. **Phase 0**: TOML Configuration System ✅
2. **Part 1, Phase 1**: CLI Refactoring ✅
3. **Part 1, Phase 2**: Background Jobs System ✅
4. **Part 2, Phase DM-1**: Download Manager Core Infrastructure
5. **Part 2, Phase DM-2**: Search Proxy
6. **Part 2, Phase DM-3**: User Request API
7. **Part 2, Phase DM-4**: Queue Processor
8. **Part 2, Phase DM-5**: Integrity Watchdog
9. **Part 2, Phase DM-6**: Admin API & Polish
10. **Part 2, Phase DM-7**: Metrics
11. **Part 2, Phase DM-8**: Documentation

### Key Design Decisions

1. **TOML overrides CLI**: Config file values take precedence over command-line arguments
2. **Download manager auto-enabled**: When `downloader_url` is configured, download manager is enabled
3. **Shared Docker network**: `pezzottify-internal` network allows catalog-server and downloader to communicate
4. **Separate database**: `download_queue.db` keeps queue state separate from catalog data
5. **Priority-based processing**: Watchdog (1) > User (2) > Expansion (3)
6. **Synchronous traits**: Following existing codebase patterns
7. **not_found errors don't retry**: Immediate failure for content that doesn't exist
8. **Parent-child queue model**: Album requests spawn child items (tracks, images) for granular tracking and easy recovery
9. **No user cancellation**: Simplifies API; admins can intervene if needed
10. **Stale items trigger alerts, not auto-fail**: Human investigation preferred over hiding issues

### Notes on Implementation

- **Trait style**: The codebase uses synchronous traits (`pub trait Foo: Send + Sync`), not `#[async_trait]`. Long-running async work is spawned as tasks.
- **Module registration**: When creating new modules, remember to add `mod` declarations in `main.rs`.
- **Permission values**: `ServerAdmin` = 7, `ViewAnalytics` = 8, `RequestContent` = 9
- **Error classification**: `not_found` → immediate FAILED; all other errors → retry with exponential backoff

### Total Tasks

| Phase | Task Count |
|-------|------------|
| Phase 0 (TOML Config) | 18 tasks ✅ |
| Part 1 (Background Jobs) | 25 tasks ✅ |
| Part 2, DM-1 (Core Infrastructure) | 38 tasks |
| Part 2, DM-2 (Search Proxy) | 7 tasks |
| Part 2, DM-3 (User Request API) | 7 tasks |
| Part 2, DM-4 (Queue Processor) | 8 tasks |
| Part 2, DM-5 (Integrity Watchdog) | 8 tasks |
| Part 2, DM-6 (Admin API & Polish) | 16 tasks |
| Part 2, DM-7 (Metrics) | 5 tasks |
| Part 2, DM-8 (Documentation) | 3 tasks |

**Total Part 2 (Download Manager): 92 tasks**

**Grand Total: ~135 tasks**
