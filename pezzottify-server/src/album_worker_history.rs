//! Durable compatibility with the former album worker's history and quotas.
use crate::{backup::DbRegistry, download_manager::QueueItem};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use simple_server::rate_limit::{evaluate_rolling_windows, RollingWindow, RollingWindowStore};
use std::{collections::HashSet, path::Path, sync::Mutex};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS runs(id INTEGER PRIMARY KEY AUTOINCREMENT, started_at TEXT NOT NULL,
 finished_at TEXT, status TEXT NOT NULL, albums_attempted INTEGER DEFAULT 0,
 albums_succeeded INTEGER DEFAULT 0, albums_failed INTEGER DEFAULT 0, error TEXT);
CREATE TABLE IF NOT EXISTS download_attempts(id INTEGER PRIMARY KEY AUTOINCREMENT,
 run_id INTEGER NOT NULL REFERENCES runs(id), queue_item_id TEXT NOT NULL, album_id TEXT NOT NULL,
 album_name TEXT, artist_name TEXT, started_at TEXT NOT NULL, finished_at TEXT, status TEXT NOT NULL,
 tracks_total INTEGER DEFAULT 0, tracks_downloaded INTEGER DEFAULT 0, ingestion_job_id TEXT,
 error TEXT, priority INTEGER, queue_created_at TEXT);
CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY AUTOINCREMENT, timestamp TEXT NOT NULL,
 run_id INTEGER, attempt_id INTEGER, level TEXT NOT NULL, category TEXT NOT NULL,
 message TEXT NOT NULL, details TEXT);
CREATE TABLE IF NOT EXISTS worker_state(key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

#[derive(Clone, Debug)]
pub(crate) struct Attempt {
    queue: String,
    album: String,
    status: String,
    started: i64,
    finished: Option<i64>,
    priority: Option<i32>,
    created: Option<i64>,
}

fn timestamp(value: &str) -> Result<i64> {
    Ok(DateTime::parse_from_rfc3339(value)
        .context("Invalid worker history timestamp")?
        .timestamp_micros())
}

pub(crate) struct History {
    connection: Mutex<Connection>,
}

impl History {
    pub(crate) fn open(path: &Path, registry: &DbRegistry) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(SCHEMA)?;
        registry.register(path.to_owned(), &connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub(crate) fn attempts(&self) -> Result<Vec<Attempt>> {
        let connection = self.connection.lock().unwrap();
        let mut query = connection.prepare("SELECT queue_item_id,album_id,status,started_at,finished_at,priority,queue_created_at FROM download_attempts")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<i32>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (queue, album, status, started, finished, priority, created) = row?;
            Ok(Attempt {
                queue,
                album,
                status,
                started: timestamp(&started)?,
                finished: finished.as_deref().map(timestamp).transpose()?,
                priority,
                created: created.as_deref().map(timestamp).transpose()?,
            })
        })
        .collect()
    }

    pub(crate) fn begin(&self, item: &QueueItem) -> Result<(i64, i64)> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        let now = Utc::now().to_rfc3339();
        transaction.execute(
            "INSERT INTO runs(started_at,status,albums_attempted) VALUES(?1,'RUNNING',1)",
            [&now],
        )?;
        let run = transaction.last_insert_rowid();
        let created = DateTime::from_timestamp(item.created_at, 0)
            .context("Invalid queue creation time")?
            .to_rfc3339();
        transaction.execute("INSERT INTO download_attempts(run_id,queue_item_id,album_id,album_name,artist_name,started_at,status,priority,queue_created_at) VALUES(?1,?2,?3,?4,?5,?6,'DOWNLOADING',?7,?8)", params![run,item.id,item.content_id,item.content_name,item.artist_name,now,item.priority as i32,created])?;
        let attempt = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok((run, attempt))
    }

    pub(crate) fn finish(
        &self,
        run: i64,
        attempt: i64,
        result: Result<&str, &str>,
        tracks: i64,
    ) -> Result<()> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        let now = Utc::now().to_rfc3339();
        let (status, job, error) = match result {
            Ok(job) => ("COMPLETED", Some(job), None),
            Err(error) => ("FAILED", None, Some(error)),
        };
        transaction.execute("UPDATE download_attempts SET status=?1,finished_at=?2,ingestion_job_id=?3,error=?4,tracks_total=?5,tracks_downloaded=CASE WHEN ?1='COMPLETED' THEN ?5 ELSE tracks_downloaded END WHERE id=?6",params![status,now,job,error,tracks,attempt])?;
        transaction.execute("UPDATE runs SET status=?1,finished_at=?2,albums_succeeded=?3,albums_failed=?4,error=?5 WHERE id=?6",params![status,now,i32::from(error.is_none()),i32::from(error.is_some()),error,run])?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn state(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM worker_state WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn set_state(&self, key: &str, value: &str) -> Result<()> {
        self.connection.lock().unwrap().execute("INSERT INTO worker_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
}

pub(crate) fn blacklisted(attempts: &[Attempt]) -> HashSet<String> {
    attempts
        .iter()
        .filter(|failed| failed.status == "FAILED")
        .filter(|failed| {
            !attempts.iter().any(|done| {
                done.album == failed.album
                    && done.status == "COMPLETED"
                    && done.finished.is_some_and(|at| at > failed.started)
            })
        })
        .filter(|failed| {
            let overtakes: HashSet<_> = attempts
                .iter()
                .filter(|done| {
                    if done.status != "COMPLETED"
                        || done.album == failed.album
                        || !done.finished.is_some_and(|at| at > failed.started)
                    {
                        return false;
                    }
                    match (failed.priority, failed.created) {
                        (Some(priority), Some(created)) => match (done.priority, done.created) {
                            (Some(p), Some(c)) => p > priority || (p == priority && c > created),
                            (None, _) => done.started > failed.started,
                            _ => false,
                        },
                        _ => done.started > failed.started,
                    }
                })
                .map(|done| &done.album)
                .collect();
            overtakes.len() >= 3
        })
        .map(|failed| failed.album.clone())
        .collect()
}

pub(crate) fn recent_completion(attempts: &[Attempt], queue: &str, now: i64) -> Option<i64> {
    attempts
        .iter()
        .filter(|a| a.queue == queue && a.status == "COMPLETED")
        .filter_map(|a| a.finished)
        .filter(|at| *at > now - 86_400_000_000)
        .max()
}

pub(crate) fn quarantine_until(attempts: &[Attempt], album: &str, now: i64) -> Option<i64> {
    let mut failures: Vec<_> = attempts
        .iter()
        .filter(|a| a.album == album && a.status == "FAILED" && a.started > now - 172_800_000_000)
        .map(|a| a.started)
        .collect();
    failures.sort_unstable();
    // Wait until fewer than three failures remain in the strict 48h window.
    (failures.len() >= 3).then(|| failures[failures.len() - 3] + 172_800_000_000)
}

struct QuotaStore<'a> {
    attempts: &'a [Attempt],
    now: i64,
}
impl RollingWindowStore for QuotaStore<'_> {
    type Error = anyhow::Error;
    fn now_micros(&mut self) -> Result<i128> {
        Ok(i128::from(self.now))
    }
    fn count_after(&mut self, _index: usize, cutoff: i128) -> Result<u64> {
        Ok(self
            .attempts
            .iter()
            .filter(|a| {
                a.status == "COMPLETED" && a.finished.is_some_and(|at| i128::from(at) > cutoff)
            })
            .count() as u64)
    }
}

pub(crate) fn quota_ready_at(attempts: &[Attempt], now: i64) -> Result<i64> {
    let windows = [
        RollingWindow {
            window_micros: 1_800_000_000,
            limit: 4,
        },
        RollingWindow {
            window_micros: 7_200_000_000,
            limit: 10,
        },
        RollingWindow {
            window_micros: 86_400_000_000,
            limit: 80,
        },
    ];
    let budget = evaluate_rolling_windows(&windows, u64::MAX, &mut QuotaStore { attempts, now })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut ready = now;
    for (window, usage) in windows.iter().zip(budget.windows) {
        if usage.remaining > 0 {
            continue;
        }
        let mut events: Vec<_> = attempts
            .iter()
            .filter(|a| a.status == "COMPLETED")
            .filter_map(|a| a.finished)
            .filter(|at| i128::from(*at) > usage.cutoff_micros)
            .collect();
        events.sort_unstable();
        let release = events[events.len() - window.limit as usize];
        ready = ready.max(release.saturating_add(window.window_micros as i64));
    }
    Ok(ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attempt(
        album: &str,
        status: &str,
        started: i64,
        finished: Option<i64>,
        priority: Option<i32>,
        created: Option<i64>,
    ) -> Attempt {
        Attempt {
            queue: album.into(),
            album: album.into(),
            status: status.into(),
            started,
            finished,
            priority,
            created,
        }
    }
    #[test]
    fn quotas_only_count_successes_strictly_after_cutoff_and_keep_future_events() {
        let now = 10_000_000_000;
        let mut attempts = vec![attempt(
            "a",
            "COMPLETED",
            0,
            Some(now - 1_800_000_000),
            None,
            None,
        )];
        attempts.extend((0..3).map(|_| attempt("b", "COMPLETED", 0, Some(now - 100), None, None)));
        attempts.push(attempt("c", "FAILED", 0, Some(now), None, None));
        assert_eq!(quota_ready_at(&attempts, now).unwrap(), now);
        attempts.push(attempt(
            "future",
            "COMPLETED",
            0,
            Some(now + 100),
            None,
            None,
        ));
        assert_eq!(
            quota_ready_at(&attempts, now).unwrap(),
            now - 100 + 1_800_000_000
        );
    }
    #[test]
    fn blacklist_counts_distinct_overtaking_albums_and_later_success_clears_it() {
        let mut attempts = vec![attempt("failed", "FAILED", 100, None, Some(1), Some(10))];
        for album in ["a", "a", "b"] {
            attempts.push(attempt(
                album,
                "COMPLETED",
                200,
                Some(300),
                Some(1),
                Some(20),
            ));
        }
        assert!(blacklisted(&attempts).is_empty());
        attempts.push(attempt("c", "COMPLETED", 200, Some(300), Some(2), Some(5)));
        assert!(blacklisted(&attempts).contains("failed"));
        attempts.push(attempt(
            "failed",
            "COMPLETED",
            400,
            Some(500),
            Some(1),
            Some(10),
        ));
        assert!(blacklisted(&attempts).is_empty());
    }
    #[test]
    fn quarantine_releases_only_after_enough_failures_age_out() {
        let attempts: Vec<_> = (1..=4)
            .map(|n| attempt("a", "FAILED", n, None, None, None))
            .collect();
        assert_eq!(quarantine_until(&attempts, "a", 10), Some(172_800_000_002));
    }
}
