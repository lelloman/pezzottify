# Background Jobs System Plan

## Overview

A recurring task scheduler for the catalog-server. This system manages periodic background tasks with configurable intervals, persistence, and monitoring.

---

## Implementation Status

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: CLI Refactoring | ✅ Complete | `--db-dir` implemented, positional args removed |
| Phase 2: Background Jobs | ✅ Complete | JobScheduler, ServerStore, PopularContentJob implemented |
| Permission Rename | ✅ Complete | `RebootServer` → `ServerAdmin` done |
| Admin API | ✅ Complete | `/v1/admin/jobs` endpoints implemented |
| Prometheus Metrics | ✅ Complete | Job execution metrics exposed |

---

## Phase 1: CLI Refactoring (Database Directory) ✅ COMPLETE

Refactor the CLI to use a single `--db-dir` argument instead of separate paths for each database. This simplifies configuration and enables adding new databases (like `server.db`) without CLI changes.

### Current CLI
```bash
cargo run -- <catalog-db-path> <user-db-path> [OPTIONS]
```

Two positional arguments for database files, which is inflexible for adding more databases.

### New CLI
```bash
cargo run -- --db-dir=<path> [OPTIONS]
```

Single directory containing all database files with well-known names.

### Database Files

| File | Description | Created if missing |
|------|-------------|-------------------|
| `catalog.db` | Catalog metadata (artists, albums, tracks) | Yes (empty schema) |
| `user.db` | User accounts, permissions, sync events | Yes (empty schema) |
| `server.db` | Server operational state (job history) | Yes (empty schema) - Phase 2 |

### Behavior

1. `--db-dir` is **required**
2. Directory **must exist** (server exits with error if not)
3. Missing `.db` files are **created automatically** with empty schema
4. Database filenames are fixed (not configurable)

### Migration Path

Existing deployments need to:
1. Create a database directory (e.g., `/data/db/`)
2. Move existing `catalog.db` and `user.db` (or whatever they were named) into that directory
3. Rename files if needed to match expected names
4. Update startup command to use `--db-dir=/data/db`

### Implementation Checklist

- [x] `catalog-server/src/main.rs`
  - Replace `catalog_db` and `user_store_file_path` positional args with `--db-dir` option
  - Add `parse_dir` function (similar to existing `parse_path`)
  - Construct `catalog.db` and `user.db` paths from `--db-dir`
  - Validate directory exists before proceeding
- [x] Docker configuration
  - Update `Dockerfile` entrypoint if hardcoded paths exist
  - Update `docker-compose.yml` volume mounts and command
- [x] Documentation
  - `catalog-server/README.md` - Update CLI usage and examples
  - `CLAUDE.md` - Update development commands
- [x] Tests
  - Update any integration tests that spawn the server with CLI args

### Example Usage After Migration

```bash
# Development
cargo run -- --db-dir=../data --media-path=../pezzottify-catalog --port=3001

# Docker
docker run -v /host/data:/data pezzottify-server --db-dir=/data --media-path=/media
```

---

## Phase 2: Background Jobs System ✅ COMPLETE

### 2.1 Scheduling Mechanisms

The system supports three scheduling types:

#### Cron-style
Standard cron expressions for precise scheduling. **All times are UTC.**

```
"0 3 * * *"     # Daily at 3:00 AM UTC
"0 */6 * * *"   # Every 6 hours
"0 0 * * 0"     # Weekly on Sunday at midnight UTC
```

#### Interval-based
Simple duration-based intervals from last completion.

```
"24h"    # Every 24 hours after last run
"30m"    # Every 30 minutes
"1d"     # Every day
```

#### Hooks (Event-driven)
Jobs triggered by system events rather than time.

| Hook Event | Description |
|------------|-------------|
| `on_startup` | Run when server starts |
| `on_catalog_change` | Run when catalog is modified |
| `on_user_created` | Run when a new user is created |
| `on_download_complete` | Run when a download finishes |

Jobs can combine hooks with schedules (e.g., run on startup AND every 24 hours).

### 2.2 Job Definition

Jobs are defined entirely in code. The database only stores execution history.

```rust
#[async_trait]
pub trait BackgroundJob: Send + Sync {
    /// Unique identifier for this job (e.g., "popular_content")
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Description of what the job does
    fn description(&self) -> &'static str;

    /// Schedule configuration
    fn schedule(&self) -> JobSchedule;

    /// Shutdown behavior: whether the job can be cancelled or must complete
    fn shutdown_behavior(&self) -> ShutdownBehavior;

    /// Execute the job asynchronously
    async fn execute(&self, ctx: JobContext) -> Result<(), JobError>;
}

/// How a job behaves during server shutdown
pub enum ShutdownBehavior {
    /// Job can be cancelled immediately via CancellationToken
    Cancellable,
    /// Job must be allowed to complete before server shuts down
    WaitForCompletion,
}

pub enum JobSchedule {
    Cron(String),                    // Cron expression
    Interval(Duration),              // Fixed interval
    Hook(HookEvent),                 // Event-triggered
    Combined {                       // Multiple triggers
        cron: Option<String>,
        interval: Option<Duration>,
        hooks: Vec<HookEvent>,
    },
}

pub enum HookEvent {
    OnStartup,
    OnCatalogChange,
    OnUserCreated,
    OnDownloadComplete,
}

pub struct JobContext {
    pub cancellation_token: CancellationToken,
    pub catalog_store: Arc<dyn CatalogStore>,
    pub user_store: Arc<SqliteUserStore>,
    // ... other dependencies as needed
}

pub enum JobError {
    /// Job not found in registry
    NotFound,
    /// Job is already running (singleton constraint)
    AlreadyRunning,
    /// Job-specific error with message
    ExecutionFailed(String),
    /// Job was cancelled via CancellationToken
    Cancelled,
    /// Job timed out
    Timeout,
}
```

### 2.3 Persistence (History Only)

The `server.db` SQLite database stores only job execution history, not job definitions.

#### Schema

```sql
-- Job execution history
CREATE TABLE job_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,              -- References job ID from code
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,              -- "running", "completed", "failed"
    error_message TEXT,
    triggered_by TEXT NOT NULL         -- "schedule", "hook:<event>", "manual"
);

-- Index for querying recent runs
CREATE INDEX idx_job_runs_job_id_started ON job_runs(job_id, started_at DESC);

-- Persists schedule state across server restarts
-- Without this, interval-based jobs would lose track of when they last ran
CREATE TABLE job_schedules (
    job_id TEXT PRIMARY KEY,
    next_run_at TEXT NOT NULL,
    last_run_at TEXT
);
```

### 2.4 Execution Model

#### Async Tasks

Jobs execute as tokio async tasks, integrating with the existing Axum runtime.

```rust
pub struct JobScheduler {
    jobs: HashMap<String, Arc<dyn BackgroundJob>>,
    running_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    server_store: Arc<ServerStore>,
}

impl JobScheduler {
    /// Spawn a job as an async task
    pub async fn trigger_job(&self, job_id: &str, triggered_by: &str) -> Result<(), JobError> {
        let job = self.jobs.get(job_id).ok_or(JobError::NotFound)?;

        // Hold lock for check+insert to prevent race between scheduler and manual triggers
        let mut running = self.running_tasks.lock().await;

        // Check if already running (singleton)
        if running.contains_key(job_id) {
            return Err(JobError::AlreadyRunning);
        }

        // Record start in database
        let run_id = self.server_store.record_job_start(job_id, triggered_by).await?;

        // Spawn async task
        let job = Arc::clone(job);
        let ctx = self.build_context();
        let store = Arc::clone(&self.server_store);
        let task_job_id = job_id.to_string();
        let running_tasks = Arc::clone(&self.running_tasks);

        let handle = tokio::spawn(async move {
            let result = job.execute(ctx).await;
            store.record_job_finish(run_id, result).await;
            // Remove from running_tasks when done
            running_tasks.lock().await.remove(&task_job_id);
        });

        running.insert(job_id.to_string(), handle);
        Ok(())
    }
}
```

#### Job Lifecycle

```
┌─────────────┐
│    Idle     │
└──────┬──────┘
       │ trigger (schedule/hook/manual)
       ▼
┌─────────────┐
│   Running   │
└──────┬──────┘
       │
       ├─────────────┐
       │ success     │ failure
       ▼             ▼
┌─────────────┐ ┌─────────────┐
│  Completed  │ │   Failed    │
└─────────────┘ └─────────────┘
```

#### Concurrency Control

- **Per-job singleton**: A job cannot run if a previous instance is still running
- No global concurrency limit (jobs are lightweight async tasks)
- Manual triggers return error if job already running

#### Hook Dispatch

Server state holds a channel for dispatching hook events to the scheduler:

```rust
// In server state
pub hook_sender: mpsc::Sender<HookEvent>,

// Scheduler holds the receiver and listens for events
pub hook_receiver: mpsc::Receiver<HookEvent>,
```

When a hook-triggering action occurs (e.g., catalog modification), the server sends the event:

```rust
// In catalog write handler
state.hook_sender.send(HookEvent::OnCatalogChange).await;
```

The scheduler receives and triggers all jobs that have that hook in their schedule.

#### Scheduler Loop

The main scheduler loop uses `tokio::select!` to wait on either:
- The next scheduled job time (cron/interval)
- An incoming hook event

```rust
impl JobScheduler {
    pub async fn run(&mut self) {
        loop {
            let sleep_duration = self.time_until_next_scheduled_job();

            tokio::select! {
                // Branch 1: Timer expires → run scheduled job(s)
                _ = tokio::time::sleep(sleep_duration) => {
                    self.run_due_jobs().await;
                }

                // Branch 2: Hook event arrives → run hook-triggered jobs
                Some(event) = self.hook_receiver.recv() => {
                    self.trigger_jobs_for_hook(event).await;
                }
            }
        }
    }

    fn time_until_next_scheduled_job(&self) -> Duration {
        // Find the earliest next_run_at across all jobs
        // Returns Duration::MAX if no scheduled jobs
    }

    async fn run_due_jobs(&self) {
        let now = Utc::now();
        for job in &self.jobs {
            if self.is_due(job, now) {
                let _ = self.trigger_job(job.id(), "schedule").await;
                self.update_next_run(job).await;
            }
        }
    }

    async fn trigger_jobs_for_hook(&self, event: HookEvent) {
        for job in &self.jobs {
            if job.schedule().has_hook(&event) {
                let _ = self.trigger_job(job.id(), &format!("hook:{:?}", event)).await;
            }
        }
    }
}
```

This approach is efficient: the scheduler sleeps until work is needed, waking on either timer expiry or hook events.

### 2.5 Admin API

Two endpoints, both requiring `ServerAdmin` permission.

#### List Jobs
```
GET /v1/admin/jobs
```

Returns all registered jobs with their current state.

Response:
```json
{
  "jobs": [
    {
      "id": "popular_content",
      "name": "Popular Content",
      "description": "Populate the popular content cache",
      "schedule": {
        "type": "interval",
        "value": "24h"
      },
      "status": "idle",
      "last_run": {
        "started_at": "2024-01-15T03:00:00Z",
        "finished_at": "2024-01-15T03:00:45Z",
        "status": "completed"
      },
      "next_run_at": "2024-01-16T03:00:00Z"
    }
  ]
}
```

Status values:
- `idle` - Not currently running
- `running` - Currently executing

Note: `last_run` is `null` if the job has never run.

#### Trigger Job Manually
```
POST /v1/admin/jobs/:job_id/trigger
```

Response (success):
```json
{
  "run_id": 124,
  "message": "Job triggered successfully"
}
```

Response (already running):
```json
{
  "error": "Job is already running"
}
```
(HTTP 409 Conflict)

### 2.6 Permission Changes ✅ COMPLETE

Rename `RebootServer` to `ServerAdmin` to encompass broader server administration capabilities:

| Old Permission | New Permission | Capabilities |
|---------------|----------------|--------------|
| `RebootServer` | `ServerAdmin` | Reboot server, manage background jobs, view server status |

**Migration:**
- Permission int value remains `7`
- Update all references in code
- Sync event log maintains compatibility (clients with old permission name continue working)

**Affected files:**
- `catalog-server/src/user/permissions.rs`
- `catalog-server/src/server/server.rs`
- `catalog-server/README.md`
- `web/src/store/user.js`
- `web/src/store/remote.js`
- `web/src/views/AdminView.vue`
- `web/src/components/admin/UserManagement.vue`
- `android/ui/src/main/java/.../Permission.kt`
- `android/domain/src/main/java/.../SyncEvent.kt`
- `android/app/src/main/java/.../InteractorsModule.kt`

### 2.7 Alerting

Job failures should trigger alerts. Integration with the existing alerting system (Alertmanager/Telegram):

- **Alert on job failure:** When a job's `execute()` returns `Err(JobError)`
- **Alert on prolonged running:** When a job exceeds a configurable duration threshold
- **Metrics exposed:** Job execution counts, durations, and failure counts via Prometheus

### 2.8 Initial Job

| Job ID | Name | Schedule | Description |
|--------|------|----------|-------------|
| `popular_content` | Popular Content | Combined: `on_startup` + Interval `24h` | Pre-compute popular content cache for fast API responses |

**Rationale:** Pre-computing the popular content cache ensures the first client request is served instantly, rather than waiting for an expensive query. The `on_startup` hook ensures the cache is warm immediately after server restart.

### 2.9 Startup and Shutdown

#### Startup

The scheduler and HTTP server run concurrently via `tokio::select!`:

```rust
// In main.rs
let scheduler = JobScheduler::new(...);

tokio::select! {
    result = run_server(...) => { /* server stopped */ },
    result = scheduler.run() => { /* scheduler stopped */ },
}
```

On startup, before entering the main loop, the scheduler:

1. **Recovers stale state:** Query `job_runs` for entries with `status = "running"`. Mark them as `failed` with `error_message = "Server crashed during execution"`.

2. **Restores schedule state:** Load `job_schedules` to determine `next_run_at` for interval-based jobs. If a job's `next_run_at` is in the past, trigger it immediately.

3. **Triggers `on_startup` hooks:** Fire all jobs with `OnStartup` in their schedule. These run as background tasks and do **not** block the server from accepting requests.

#### Shutdown

When the server receives a shutdown signal (SIGTERM/SIGINT):

1. Stop accepting new HTTP requests
2. Cancel the scheduler loop
3. For each running job:
   - If `ShutdownBehavior::Cancellable`: signal cancellation via `CancellationToken`
   - If `ShutdownBehavior::WaitForCompletion`: wait for the job to finish
4. Exit once all `WaitForCompletion` jobs are done (with a timeout to prevent hanging)

### 2.10 Future Jobs

- `event_pruning` - Prune old sync events (replaces current hardcoded task in main.rs)
- `job_history_cleanup` - Remove old job run history from `job_runs` table
- `integrity_watchdog` - Scan catalog for missing files
- `catalog_expansion_agent` - Automatically expand catalog based on listening patterns
- `analytics_aggregation` - Aggregate listening stats for reporting

---

## Dependencies

None - job definitions and schedules are entirely in code.

## Used By

- Popular content feature
- Future features (integrity watchdog, expansion agent, etc.)

---

## Implementation Summary

### Phase 1: CLI Refactoring
1. Replace positional db args with `--db-dir` option
2. Update Docker configuration
3. Update documentation

### Phase 2: Background Jobs
1. Create `server.db` schema and `ServerStore`
2. Rename `RebootServer` → `ServerAdmin` permission
3. Implement `JobScheduler` with async execution
4. Add Admin API endpoints (list jobs, trigger job)
5. Implement `popular_content` job
