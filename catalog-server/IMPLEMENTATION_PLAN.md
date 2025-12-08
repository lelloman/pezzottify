# Detailed Implementation Plan

This document breaks down the Background Jobs System and Download Manager plans into sequential, actionable tasks.

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

- [ ] **Task 2.2.1: Create `server_store` module structure**

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

- [ ] **Task 2.2.2: Define `JobRun` and `JobScheduleState` models**

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

- [ ] **Task 2.4.1: Create `GET /v1/admin/jobs` handler**

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

- [ ] **Task 2.4.2: Create `POST /v1/admin/jobs/:job_id/trigger` handler**

  **Context:** Manually trigger a job. Returns 409 if already running.

  **File:** `catalog-server/src/server/server.rs`

- [ ] **Task 2.4.3: Add `require_server_admin` middleware**

  **Context:** Rename/update existing `require_reboot_server` middleware to `require_server_admin`.

  **File:** `catalog-server/src/server/server.rs`

- [ ] **Task 2.4.4: Wire routes into router**

  **File:** `catalog-server/src/server/server.rs`

  **Add:**
  ```rust
  .route("/v1/admin/jobs", get(list_jobs))
  .route("/v1/admin/jobs/:job_id/trigger", post(trigger_job))
  ```

#### 2.5 Main Integration

**Goal:** Initialize and run the scheduler alongside the HTTP server.

- [ ] **Task 2.5.1: Initialize `SqliteServerStore` in main.rs**

  **File:** `catalog-server/src/main.rs`

  **Sample:**
  ```rust
  let server_store = Arc::new(SqliteServerStore::new(&config.server_db_path())?);
  ```

- [ ] **Task 2.5.2: Initialize `JobScheduler` with registered jobs**

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

- [ ] **Task 2.5.3: Run scheduler with `tokio::select!` alongside HTTP server**

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

- [ ] **Task 2.6.1: Create `popular_content` job**

  **File:** `catalog-server/src/background_jobs/jobs/popular_content.rs`

- [ ] **Task 2.6.2: Register `popular_content` job in scheduler**

  **File:** `catalog-server/src/main.rs`

- [ ] **Task 2.6.3: Add tests for `popular_content` job**

  **File:** `catalog-server/src/background_jobs/jobs/popular_content.rs`

#### 2.7 Metrics

- [ ] **Task 2.7.1: Add Prometheus metrics for job execution**

  **File:** `catalog-server/src/server/metrics.rs`

  **Metrics:**
  - `pezzottify_background_job_executions_total{job_id, status}`
  - `pezzottify_background_job_duration_seconds{job_id}`
  - `pezzottify_background_job_running{job_id}`

- [ ] **Task 2.7.2: Emit metrics from scheduler**

  **File:** `catalog-server/src/background_jobs/scheduler.rs`

---

## Part 2: Download Manager

**Prerequisites:** Background Jobs System (Phase 2)

### Phase DM-1: Core Infrastructure

#### DM-1.1 Module Structure

- [ ] **Task DM-1.1.1: Create `download_manager` module**

  **Files to create:**
  - `catalog-server/src/download_manager/mod.rs`
  - `catalog-server/src/download_manager/models.rs`
  - `catalog-server/src/download_manager/queue_store.rs`
  - `catalog-server/src/download_manager/audit_logger.rs`
  - `catalog-server/src/download_manager/retry_policy.rs`
  - `catalog-server/src/download_manager/schema.rs`

  **Add to `main.rs`:**
  ```rust
  mod download_manager;
  ```

#### DM-1.2 Models

- [ ] **Task DM-1.2.1: Define queue enums**

  **File:** `catalog-server/src/download_manager/models.rs`

  **Sample:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum QueueStatus {
      Pending,
      InProgress,
      RetryWaiting,
      Completed,
      Failed,
      Cancelled,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  pub enum QueuePriority {
      Watchdog = 1,   // Highest priority - integrity repairs
      User = 2,       // User requests
      Expansion = 3,  // Auto-expansion, discography fills
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum DownloadContentType {
      Album,
      TrackAudio,
      ArtistImage,
      AlbumImage,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum RequestSource {
      UserRequest,    // Explicit user request
      Watchdog,       // Integrity watchdog repair
      Expansion,      // Auto-expansion (e.g., related content)
  }
  ```

- [ ] **Task DM-1.2.2: Define `QueueItem` struct**

  **File:** `catalog-server/src/download_manager/models.rs`

- [ ] **Task DM-1.2.3: Define audit types**

  **File:** `catalog-server/src/download_manager/models.rs`

  **Sample:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AuditEventType {
      RequestCreated,
      DownloadStarted,
      DownloadCompleted,
      DownloadFailed,
      RetryScheduled,
      RequestCancelled,
      AdminRetry,
      WatchdogQueued,
      WatchdogScanStarted,
      WatchdogScanCompleted,
  }

  #[derive(Debug, Clone)]
  pub struct AuditLogEntry {
      pub id: i64,
      pub queue_item_id: Option<String>,
      pub user_id: Option<String>,
      pub event_type: AuditEventType,
      pub details: Option<String>,  // JSON string
      pub created_at: i64,
  }
  ```

#### DM-1.3 Database Schema

- [ ] **Task DM-1.3.1: Create `download_queue.db` schema**

  **File:** `catalog-server/src/download_manager/schema.rs`

- [ ] **Task DM-1.3.2: Implement schema migration**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

#### DM-1.4 Queue Store

- [ ] **Task DM-1.4.1: Define `DownloadQueueStore` trait**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

- [ ] **Task DM-1.4.2: Implement `SqliteDownloadQueueStore`**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

- [ ] **Task DM-1.4.3: Add unit tests for queue store**

  **File:** `catalog-server/src/download_manager/queue_store.rs`

#### DM-1.5 Support Components

- [ ] **Task DM-1.5.1: Implement `AuditLogger`**

  **File:** `catalog-server/src/download_manager/audit_logger.rs`

- [ ] **Task DM-1.5.2: Implement `RetryPolicy`**

  **File:** `catalog-server/src/download_manager/retry_policy.rs`

#### DM-1.6 Permission

- [ ] **Task DM-1.6.1: Add `RequestContent` permission (ID = 9)**

  **File:** `catalog-server/src/user/permissions.rs`

- [ ] **Task DM-1.6.2: Update Admin role to include RequestContent**

  **File:** `catalog-server/src/user/permissions.rs`

- [ ] **Task DM-1.6.3: Update permission documentation**

  **File:** `catalog-server/README.md`

---

### Phase DM-2: Search Proxy

- [ ] **Task DM-2.1: Verify downloader service has search endpoints**

- [ ] **Task DM-2.2: Create `search_proxy.rs` module**

  **File:** `catalog-server/src/download_manager/search_proxy.rs`

- [ ] **Task DM-2.3: Implement search with `in_catalog`/`in_queue` enrichment**

- [ ] **Task DM-2.4: Create API handlers**

  - `GET /v1/download/search`
  - `GET /v1/download/search/discography/:artist_id`

---

### Phase DM-3: User Request API

- [ ] **Task DM-3.1: Create request handlers**

  - `POST /v1/download/request/album`
  - `POST /v1/download/request/discography`
  - `GET /v1/download/my-requests`
  - `GET /v1/download/request/:id`
  - `DELETE /v1/download/request/:id`

- [ ] **Task DM-3.2: Implement rate limiting**

- [ ] **Task DM-3.3: Add `require_request_content` middleware**

---

### Phase DM-4: Queue Processor

- [ ] **Task DM-4.1: Create `job_processor.rs`**

  **File:** `catalog-server/src/download_manager/job_processor.rs`

- [ ] **Task DM-4.2: Integrate with `Downloader` trait**

- [ ] **Task DM-4.3: Spawn processor background task in main.rs**

- [ ] **Task DM-4.4: Add download queue metrics**

---

### Phase DM-5: Integrity Watchdog

- [ ] **Task DM-5.1: Create `watchdog.rs`**

  **File:** `catalog-server/src/download_manager/watchdog.rs`

- [ ] **Task DM-5.2: Implement scan methods**

  - `scan_missing_track_audio`
  - `scan_missing_album_images`
  - `scan_missing_artist_images`

- [ ] **Task DM-5.3: Create `IntegrityWatchdogJob` background job**

  **File:** `catalog-server/src/background_jobs/jobs/integrity_watchdog.rs`

- [ ] **Task DM-5.4: Register watchdog job (conditional on download manager enabled)**

---

### Phase DM-6: Admin API & Polish

- [ ] **Task DM-6.1: Create admin handlers**

  - `GET /v1/download/admin/stats`
  - `GET /v1/download/admin/failed`
  - `POST /v1/download/admin/retry/:id`
  - `GET /v1/download/admin/activity`
  - `GET /v1/download/admin/requests`

- [ ] **Task DM-6.2: Create audit log handlers**

  - `GET /v1/download/admin/audit`
  - `GET /v1/download/admin/audit/item/:queue_item_id`
  - `GET /v1/download/admin/audit/user/:user_id`

- [ ] **Task DM-6.3: Implement audit log cleanup job**

- [ ] **Task DM-6.4: Create `download_routes` function and wire into router**

- [ ] **Task DM-6.5: Write integration tests**

---

## Summary

### Phase Order (Dependencies)

1. **Phase 0**: TOML Configuration System
2. **Part 1, Phase 1**: CLI Refactoring (minimal, mostly covered in Phase 0)
3. **Part 1, Phase 2**: Background Jobs System
4. **Part 2, Phase DM-1**: Download Manager Core Infrastructure
5. **Part 2, Phase DM-2**: Search Proxy
6. **Part 2, Phase DM-3**: User Request API
7. **Part 2, Phase DM-4**: Queue Processor
8. **Part 2, Phase DM-5**: Integrity Watchdog (requires Background Jobs)
9. **Part 2, Phase DM-6**: Admin API & Polish

### Key Design Decisions

1. **TOML overrides CLI**: Config file values take precedence over command-line arguments
2. **Download manager auto-enabled**: When `downloader_url` is configured, download manager is enabled
3. **Shared Docker network**: `pezzottify-internal` network allows catalog-server and downloader to communicate without referencing each other
4. **Network creation**: First service to start creates the network (idempotent via `name:` field)
5. **Synchronous traits**: Following existing codebase patterns, traits are synchronous. Async operations use `spawn_blocking` when needed.

### Notes on Implementation

- **Trait style**: The codebase uses synchronous traits (`pub trait Foo: Send + Sync`), not `#[async_trait]`. Long-running async work is spawned as tasks.
- **Module registration**: When creating new modules, remember to add `mod` declarations in `main.rs` (and `cli_auth.rs` if shared).
- **Permission values**: `RebootServer`/`ServerAdmin` = 7, `ViewAnalytics` = 8, `RequestContent` = 9

### Total Tasks

- Phase 0 (TOML Config): ~18 tasks
- Part 1 (Background Jobs): ~25 tasks
- Part 2 (Download Manager): ~30 tasks (condensed from detailed breakdown)

**Total: ~73 high-level tasks** (expanded to ~120 when including subtasks)
