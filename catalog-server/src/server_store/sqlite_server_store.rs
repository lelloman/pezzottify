use super::models::{JobRun, JobRunStatus, JobScheduleState};
use super::schema::SERVER_VERSIONED_SCHEMAS;
use super::ServerStore;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

const BASE_DB_VERSION: usize = 99999;

pub struct SqliteServerStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteServerStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path = db_path.as_ref();
        let conn = Connection::open(path).context("Failed to open server database")?;

        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // Check user_version to determine if this is a fresh database
        let raw_version: i64 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if raw_version == 0 {
            // Fresh database - initialize schema
            info!("Creating new server database at {:?}", path);
            Self::initialize_schema(&conn)?;
        } else {
            // Existing database - check version and migrate if needed
            let db_version = raw_version - BASE_DB_VERSION as i64;

            if db_version < 1 {
                anyhow::bail!(
                    "Server database version {} is invalid (expected >= 1)",
                    db_version
                );
            }

            let current_schema_version = SERVER_VERSIONED_SCHEMAS.last().unwrap().version as i64;
            if db_version < current_schema_version {
                info!(
                    "Migrating server database from version {} to {}",
                    db_version, current_schema_version
                );
                Self::run_migrations(&conn, db_version as usize)?;
            }
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn initialize_schema(conn: &Connection) -> Result<()> {
        let schema = SERVER_VERSIONED_SCHEMAS
            .last()
            .expect("No schemas defined");
        conn.execute_batch(schema.up)
            .context("Failed to initialize server database schema")?;
        conn.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + schema.version),
            [],
        )?;
        Ok(())
    }

    fn run_migrations(conn: &Connection, from_version: usize) -> Result<()> {
        for schema in SERVER_VERSIONED_SCHEMAS.iter() {
            if schema.version > from_version {
                info!("Running server database migration to version {}", schema.version);
                conn.execute_batch(schema.up)
                    .with_context(|| format!("Failed to run migration to version {}", schema.version))?;
                conn.execute(
                    &format!("PRAGMA user_version = {}", BASE_DB_VERSION + schema.version),
                    [],
                )?;
            }
        }
        Ok(())
    }

    fn format_datetime(dt: &DateTime<Utc>) -> String {
        dt.to_rfc3339()
    }

    fn row_to_job_run(row: &rusqlite::Row) -> rusqlite::Result<JobRun> {
        let status_str: String = row.get("status")?;
        let status = JobRunStatus::from_str(&status_str).unwrap_or(JobRunStatus::Failed);

        let started_at_str: String = row.get("started_at")?;
        let finished_at_str: Option<String> = row.get("finished_at")?;

        Ok(JobRun {
            id: row.get("id")?,
            job_id: row.get("job_id")?,
            started_at: DateTime::parse_from_rfc3339(&started_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            finished_at: finished_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            status,
            error_message: row.get("error_message")?,
            triggered_by: row.get("triggered_by")?,
        })
    }

    fn row_to_schedule_state(row: &rusqlite::Row) -> rusqlite::Result<JobScheduleState> {
        let next_run_at_str: String = row.get("next_run_at")?;
        let last_run_at_str: Option<String> = row.get("last_run_at")?;

        Ok(JobScheduleState {
            job_id: row.get("job_id")?,
            next_run_at: DateTime::parse_from_rfc3339(&next_run_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_run_at: last_run_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
        })
    }
}

impl ServerStore for SqliteServerStore {
    fn record_job_start(&self, job_id: &str, triggered_by: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        conn.execute(
            "INSERT INTO job_runs (job_id, started_at, status, triggered_by)
             VALUES (?1, ?2, ?3, ?4)",
            params![job_id, now, JobRunStatus::Running.as_str(), triggered_by],
        )?;

        Ok(conn.last_insert_rowid())
    }

    fn record_job_finish(
        &self,
        run_id: i64,
        status: JobRunStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        conn.execute(
            "UPDATE job_runs SET finished_at = ?1, status = ?2, error_message = ?3 WHERE id = ?4",
            params![now, status.as_str(), error_message, run_id],
        )?;

        Ok(())
    }

    fn get_running_jobs(&self) -> Result<Vec<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE status = ?1 ORDER BY started_at DESC",
        )?;

        let jobs = stmt
            .query_map(params![JobRunStatus::Running.as_str()], Self::row_to_job_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(jobs)
    }

    fn get_job_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;

        let jobs = stmt
            .query_map(params![job_id, limit as i64], Self::row_to_job_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(jobs)
    }

    fn get_last_run(&self, job_id: &str) -> Result<Option<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT 1",
        )?;

        let job = stmt
            .query_row(params![job_id], Self::row_to_job_run)
            .optional()?;

        Ok(job)
    }

    fn mark_stale_jobs_failed(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        // Mark any jobs that are still "running" as failed
        // This is called at startup to clean up jobs that were interrupted
        let count = conn.execute(
            "UPDATE job_runs SET status = ?1, finished_at = ?2, error_message = ?3
             WHERE status = ?4",
            params![
                JobRunStatus::Failed.as_str(),
                now,
                "Job was interrupted (server restart)",
                JobRunStatus::Running.as_str()
            ],
        )?;

        Ok(count)
    }

    fn get_schedule_state(&self, job_id: &str) -> Result<Option<JobScheduleState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT job_id, next_run_at, last_run_at FROM job_schedules WHERE job_id = ?1",
        )?;

        let state = stmt
            .query_row(params![job_id], Self::row_to_schedule_state)
            .optional()?;

        Ok(state)
    }

    fn update_schedule_state(&self, state: &JobScheduleState) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let next_run_at = Self::format_datetime(&state.next_run_at);
        let last_run_at = state.last_run_at.as_ref().map(Self::format_datetime);

        conn.execute(
            "INSERT INTO job_schedules (job_id, next_run_at, last_run_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(job_id) DO UPDATE SET next_run_at = ?2, last_run_at = ?3",
            params![state.job_id, next_run_at, last_run_at],
        )?;

        Ok(())
    }

    fn get_all_schedule_states(&self) -> Result<Vec<JobScheduleState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT job_id, next_run_at, last_run_at FROM job_schedules")?;

        let states = stmt
            .query_map([], Self::row_to_schedule_state)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestStore {
        store: SqliteServerStore,
        _temp_dir: TempDir, // Keep temp dir alive
    }

    fn create_test_store() -> TestStore {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("server.db");
        let store = SqliteServerStore::new(&db_path).unwrap();
        TestStore {
            store,
            _temp_dir: temp_dir,
        }
    }

    #[test]
    fn test_record_job_start_and_finish() {
        let test = create_test_store();
        let store = &test.store;

        // Start a job
        let run_id = store.record_job_start("test_job", "manual").unwrap();
        assert!(run_id > 0);

        // Check it's running
        let running = store.get_running_jobs().unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].job_id, "test_job");
        assert_eq!(running[0].status, JobRunStatus::Running);

        // Finish the job
        store
            .record_job_finish(run_id, JobRunStatus::Completed, None)
            .unwrap();

        // Check it's no longer running
        let running = store.get_running_jobs().unwrap();
        assert!(running.is_empty());

        // Check history
        let history = store.get_job_history("test_job", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, JobRunStatus::Completed);
        assert!(history[0].finished_at.is_some());
    }

    #[test]
    fn test_record_job_failure_with_error() {
        let test = create_test_store();
        let store = &test.store;

        let run_id = store.record_job_start("failing_job", "schedule").unwrap();
        store
            .record_job_finish(
                run_id,
                JobRunStatus::Failed,
                Some("Something went wrong".to_string()),
            )
            .unwrap();

        let last_run = store.get_last_run("failing_job").unwrap().unwrap();
        assert_eq!(last_run.status, JobRunStatus::Failed);
        assert_eq!(
            last_run.error_message,
            Some("Something went wrong".to_string())
        );
    }

    #[test]
    fn test_get_job_history_limit() {
        let test = create_test_store();
        let store = &test.store;

        // Create multiple job runs
        for i in 0..5 {
            let run_id = store
                .record_job_start("history_job", &format!("run_{}", i))
                .unwrap();
            store
                .record_job_finish(run_id, JobRunStatus::Completed, None)
                .unwrap();
        }

        // Get limited history
        let history = store.get_job_history("history_job", 3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_mark_stale_jobs_failed() {
        let test = create_test_store();
        let store = &test.store;

        // Start jobs but don't finish them
        store.record_job_start("stale_job_1", "schedule").unwrap();
        store.record_job_start("stale_job_2", "hook").unwrap();

        // Mark stale jobs as failed
        let count = store.mark_stale_jobs_failed().unwrap();
        assert_eq!(count, 2);

        // Verify they're now failed
        let running = store.get_running_jobs().unwrap();
        assert!(running.is_empty());

        let last_run = store.get_last_run("stale_job_1").unwrap().unwrap();
        assert_eq!(last_run.status, JobRunStatus::Failed);
        assert!(last_run
            .error_message
            .unwrap()
            .contains("server restart"));
    }

    #[test]
    fn test_schedule_state_crud() {
        let test = create_test_store();
        let store = &test.store;

        // Initially no state
        let state = store.get_schedule_state("scheduled_job").unwrap();
        assert!(state.is_none());

        // Create state
        let new_state = JobScheduleState {
            job_id: "scheduled_job".to_string(),
            next_run_at: Utc::now(),
            last_run_at: None,
        };
        store.update_schedule_state(&new_state).unwrap();

        // Read state
        let state = store.get_schedule_state("scheduled_job").unwrap().unwrap();
        assert_eq!(state.job_id, "scheduled_job");
        assert!(state.last_run_at.is_none());

        // Update state
        let updated_state = JobScheduleState {
            job_id: "scheduled_job".to_string(),
            next_run_at: Utc::now(),
            last_run_at: Some(Utc::now()),
        };
        store.update_schedule_state(&updated_state).unwrap();

        let state = store.get_schedule_state("scheduled_job").unwrap().unwrap();
        assert!(state.last_run_at.is_some());
    }

    #[test]
    fn test_get_all_schedule_states() {
        let test = create_test_store();
        let store = &test.store;

        // Create multiple schedule states
        for i in 0..3 {
            let state = JobScheduleState {
                job_id: format!("job_{}", i),
                next_run_at: Utc::now(),
                last_run_at: None,
            };
            store.update_schedule_state(&state).unwrap();
        }

        let states = store.get_all_schedule_states().unwrap();
        assert_eq!(states.len(), 3);
    }

    #[test]
    fn test_get_last_run_nonexistent_job() {
        let test = create_test_store();
        let store = &test.store;
        let last_run = store.get_last_run("nonexistent").unwrap();
        assert!(last_run.is_none());
    }
}
