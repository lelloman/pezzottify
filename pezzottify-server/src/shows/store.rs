use super::{Show, ShowSegment, ShowSource, ShowSpeaker, ShowStatus, ShowSummary};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub trait ShowStore: Send + Sync {
    fn upsert_show(&self, show: &Show) -> Result<()>;
    fn get_show(&self, id: &str) -> Result<Option<Show>>;
    fn list_published(&self, limit: usize, offset: usize) -> Result<Vec<ShowSummary>>;
    fn list_admin(&self, limit: usize, offset: usize) -> Result<Vec<ShowSummary>>;
    fn delete_show(&self, id: &str) -> Result<bool>;
}

pub struct SqliteShowStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteShowStore {
    pub fn open<P: AsRef<Path>>(path: P, db_registry: &crate::backup::DbRegistry) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).context("Failed to open shows database")?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        Self::create_schema(&conn)?;
        db_registry.register(path.to_path_buf(), &conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn create_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS shows (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                brief TEXT NOT NULL,
                summary TEXT NOT NULL,
                language TEXT NOT NULL,
                target_duration_minutes INTEGER NOT NULL,
                created_by_user_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                published_at INTEGER,
                speakers_json TEXT NOT NULL,
                segments_json TEXT NOT NULL,
                sources_json TEXT NOT NULL,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_shows_status_updated ON shows(status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_shows_published_at ON shows(published_at DESC);",
        )?;
        Ok(())
    }

    fn row_to_show(row: &rusqlite::Row) -> rusqlite::Result<Show> {
        let status_str: String = row.get("status")?;
        let speakers_json: String = row.get("speakers_json")?;
        let segments_json: String = row.get("segments_json")?;
        let sources_json: String = row.get("sources_json")?;
        let created_by_user_id: i64 = row.get("created_by_user_id")?;
        Ok(Show {
            id: row.get("id")?,
            title: row.get("title")?,
            status: ShowStatus::parse(&status_str).unwrap_or(ShowStatus::Failed),
            brief: row.get("brief")?,
            summary: row.get("summary")?,
            language: row.get("language")?,
            target_duration_minutes: row.get("target_duration_minutes")?,
            created_by_user_id: created_by_user_id as usize,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            published_at: row.get("published_at")?,
            speakers: serde_json::from_str::<Vec<ShowSpeaker>>(&speakers_json).unwrap_or_default(),
            segments: serde_json::from_str::<Vec<ShowSegment>>(&segments_json).unwrap_or_default(),
            sources: serde_json::from_str::<Vec<ShowSource>>(&sources_json).unwrap_or_default(),
            error: row.get("error")?,
        })
    }
}

impl ShowStore for SqliteShowStore {
    fn upsert_show(&self, show: &Show) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO shows (
                id, title, status, brief, summary, language, target_duration_minutes,
                created_by_user_id, created_at, updated_at, published_at,
                speakers_json, segments_json, sources_json, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(id) DO UPDATE SET
                title=excluded.title,
                status=excluded.status,
                brief=excluded.brief,
                summary=excluded.summary,
                language=excluded.language,
                target_duration_minutes=excluded.target_duration_minutes,
                updated_at=excluded.updated_at,
                published_at=excluded.published_at,
                speakers_json=excluded.speakers_json,
                segments_json=excluded.segments_json,
                sources_json=excluded.sources_json,
                error=excluded.error",
            params![
                show.id,
                show.title,
                show.status.as_str(),
                show.brief,
                show.summary,
                show.language,
                show.target_duration_minutes,
                show.created_by_user_id as i64,
                show.created_at,
                show.updated_at,
                show.published_at,
                serde_json::to_string(&show.speakers)?,
                serde_json::to_string(&show.segments)?,
                serde_json::to_string(&show.sources)?,
                show.error,
            ],
        )?;
        Ok(())
    }

    fn get_show(&self, id: &str) -> Result<Option<Show>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM shows WHERE id = ?1",
            params![id],
            Self::row_to_show,
        )
        .optional()
        .map_err(Into::into)
    }

    fn list_published(&self, limit: usize, offset: usize) -> Result<Vec<ShowSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM shows WHERE status = 'published' ORDER BY published_at DESC, updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], Self::row_to_show)?;
        let mut out = Vec::new();
        for row in rows {
            let show = row?;
            out.push(ShowSummary::from(&show));
        }
        Ok(out)
    }

    fn list_admin(&self, limit: usize, offset: usize) -> Result<Vec<ShowSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM shows ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2")?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], Self::row_to_show)?;
        let mut out = Vec::new();
        for row in rows {
            let show = row?;
            out.push(ShowSummary::from(&show));
        }
        Ok(out)
    }

    fn delete_show(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute("DELETE FROM shows WHERE id = ?1", params![id])? > 0)
    }
}
