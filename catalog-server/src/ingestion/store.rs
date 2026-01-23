//! SQLite store for ingestion data.
//!
//! Album-first storage with:
//! - Jobs (album-level)
//! - Files (individual audio files within a job)
//! - Reasoning log
//! - Review queue

use super::models::{
    ConversionReason, IngestionContextType, IngestionFile, IngestionJob, IngestionJobStatus,
    IngestionMatchSource, ReviewQueueItem,
};
use super::schema::INGESTION_SCHEMA_SQL;
use crate::agent::reasoning::ReasoningStep;
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Trait for ingestion storage operations.
pub trait IngestionStore: Send + Sync {
    // ==================== Job Operations ====================

    /// Create a new ingestion job.
    fn create_job(&self, job: &IngestionJob) -> Result<()>;

    /// Get a job by ID.
    fn get_job(&self, id: &str) -> Result<Option<IngestionJob>>;

    /// Update a job.
    fn update_job(&self, job: &IngestionJob) -> Result<()>;

    /// Delete a job and all associated data.
    fn delete_job(&self, id: &str) -> Result<()>;

    /// List jobs for a user.
    fn list_jobs_by_user(&self, user_id: &str, limit: usize) -> Result<Vec<IngestionJob>>;

    /// List jobs by status.
    fn list_jobs_by_status(
        &self,
        status: IngestionJobStatus,
        limit: usize,
    ) -> Result<Vec<IngestionJob>>;

    /// List all jobs (for admin).
    fn list_all_jobs(&self, limit: usize) -> Result<Vec<IngestionJob>>;

    // ==================== File Operations ====================

    /// Create a new file record.
    fn create_file(&self, file: &IngestionFile) -> Result<()>;

    /// Get a file by ID.
    fn get_file(&self, id: &str) -> Result<Option<IngestionFile>>;

    /// Update a file record.
    fn update_file(&self, file: &IngestionFile) -> Result<()>;

    /// Get all files for a job.
    fn get_files_for_job(&self, job_id: &str) -> Result<Vec<IngestionFile>>;

    /// Delete files for a job.
    fn delete_files_for_job(&self, job_id: &str) -> Result<()>;

    // ==================== Reasoning Log ====================

    /// Log a reasoning step.
    fn log_reasoning_step(&self, job_id: &str, step: &ReasoningStep) -> Result<()>;

    /// Get reasoning steps for a job.
    fn get_reasoning_steps(&self, job_id: &str) -> Result<Vec<ReasoningStep>>;

    // ==================== Review Queue ====================

    /// Create a review queue item.
    fn create_review_item(&self, job_id: &str, question: &str, options: &str) -> Result<()>;

    /// Get pending review items.
    fn get_pending_reviews(&self, limit: usize) -> Result<Vec<ReviewQueueItem>>;

    /// Resolve a review item.
    fn resolve_review(&self, job_id: &str, user_id: &str, selected_option: &str) -> Result<()>;

    /// Get review item for a job.
    fn get_review_item(&self, job_id: &str) -> Result<Option<ReviewQueueItem>>;
}

/// SQLite implementation of IngestionStore.
pub struct SqliteIngestionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteIngestionStore {
    /// Open or create an ingestion database.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open ingestion database: {:?}", path))?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        // Apply schema
        conn.execute_batch(INGESTION_SCHEMA_SQL)?;

        // Run migrations if needed
        super::schema::migrate_v2_to_v3(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory database (for testing).
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.execute_batch(INGESTION_SCHEMA_SQL)?;
        super::schema::migrate_v2_to_v3(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<IngestionJob> {
        Ok(IngestionJob {
            id: row.get("id")?,
            status: IngestionJobStatus::parse(&row.get::<_, String>("status")?)
                .unwrap_or(IngestionJobStatus::Pending),
            user_id: row.get("user_id")?,
            original_filename: row.get("original_filename")?,
            total_size_bytes: row.get("total_size_bytes")?,
            file_count: row.get("file_count")?,
            context_type: row
                .get::<_, Option<String>>("context_type")?
                .and_then(|s| IngestionContextType::parse(&s)),
            context_id: row.get("context_id")?,
            detected_artist: row.get("detected_artist")?,
            detected_album: row.get("detected_album")?,
            detected_year: row.get("detected_year")?,
            matched_album_id: row.get("matched_album_id")?,
            match_confidence: row.get("match_confidence")?,
            match_source: row
                .get::<_, Option<String>>("match_source")?
                .and_then(|s| IngestionMatchSource::parse(&s)),
            tracks_matched: row.get("tracks_matched")?,
            tracks_converted: row.get("tracks_converted")?,
            error_message: row.get("error_message")?,
            created_at: row.get("created_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            workflow_state: row.get("workflow_state")?,
        })
    }

    fn row_to_file(row: &rusqlite::Row) -> rusqlite::Result<IngestionFile> {
        // Parse conversion_reason from JSON if present
        let conversion_reason: Option<ConversionReason> = row
            .get::<_, Option<String>>("conversion_reason")?
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(IngestionFile {
            id: row.get("id")?,
            job_id: row.get("job_id")?,
            filename: row.get("filename")?,
            file_size_bytes: row.get("file_size_bytes")?,
            temp_file_path: row.get("temp_file_path")?,
            duration_ms: row.get("duration_ms")?,
            codec: row.get("codec")?,
            bitrate: row.get("bitrate")?,
            sample_rate: row.get("sample_rate")?,
            tag_artist: row.get("tag_artist")?,
            tag_album: row.get("tag_album")?,
            tag_title: row.get("tag_title")?,
            tag_track_num: row.get("tag_track_num")?,
            tag_track_total: row.get("tag_track_total")?,
            tag_disc_num: row.get("tag_disc_num")?,
            tag_year: row.get("tag_year")?,
            matched_track_id: row.get("matched_track_id")?,
            match_confidence: row.get("match_confidence")?,
            output_file_path: row.get("output_file_path")?,
            converted: row.get::<_, i32>("converted")? != 0,
            error_message: row.get("error_message")?,
            conversion_reason,
            created_at: row.get("created_at")?,
        })
    }
}

impl IngestionStore for SqliteIngestionStore {
    // ==================== Job Operations ====================

    fn create_job(&self, job: &IngestionJob) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO ingestion_jobs (
                id, status, user_id, original_filename, total_size_bytes, file_count,
                context_type, context_id,
                detected_artist, detected_album, detected_year,
                matched_album_id, match_confidence, match_source,
                tracks_matched, tracks_converted, error_message,
                created_at, started_at, completed_at, workflow_state
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
            )
            "#,
            params![
                job.id,
                job.status.as_str(),
                job.user_id,
                job.original_filename,
                job.total_size_bytes,
                job.file_count,
                job.context_type.map(|c| c.as_str()),
                job.context_id,
                job.detected_artist,
                job.detected_album,
                job.detected_year,
                job.matched_album_id,
                job.match_confidence,
                job.match_source.map(|m| m.as_str()),
                job.tracks_matched,
                job.tracks_converted,
                job.error_message,
                job.created_at,
                job.started_at,
                job.completed_at,
                job.workflow_state,
            ],
        )?;
        Ok(())
    }

    fn get_job(&self, id: &str) -> Result<Option<IngestionJob>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT * FROM ingestion_jobs WHERE id = ?1",
                params![id],
                Self::row_to_job,
            )
            .optional()?;
        Ok(result)
    }

    fn update_job(&self, job: &IngestionJob) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE ingestion_jobs SET
                status = ?2, file_count = ?3,
                context_type = ?4, context_id = ?5,
                detected_artist = ?6, detected_album = ?7, detected_year = ?8,
                matched_album_id = ?9, match_confidence = ?10, match_source = ?11,
                tracks_matched = ?12, tracks_converted = ?13, error_message = ?14,
                started_at = ?15, completed_at = ?16, workflow_state = ?17
            WHERE id = ?1
            "#,
            params![
                job.id,
                job.status.as_str(),
                job.file_count,
                job.context_type.map(|c| c.as_str()),
                job.context_id,
                job.detected_artist,
                job.detected_album,
                job.detected_year,
                job.matched_album_id,
                job.match_confidence,
                job.match_source.map(|m| m.as_str()),
                job.tracks_matched,
                job.tracks_converted,
                job.error_message,
                job.started_at,
                job.completed_at,
                job.workflow_state,
            ],
        )?;
        Ok(())
    }

    fn delete_job(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ingestion_jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn list_jobs_by_user(&self, user_id: &str, limit: usize) -> Result<Vec<IngestionJob>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM ingestion_jobs WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let jobs = stmt
            .query_map(params![user_id, limit as i64], Self::row_to_job)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(jobs)
    }

    fn list_jobs_by_status(
        &self,
        status: IngestionJobStatus,
        limit: usize,
    ) -> Result<Vec<IngestionJob>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM ingestion_jobs WHERE status = ?1 ORDER BY created_at ASC LIMIT ?2",
        )?;
        let jobs = stmt
            .query_map(params![status.as_str(), limit as i64], Self::row_to_job)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(jobs)
    }

    fn list_all_jobs(&self, limit: usize) -> Result<Vec<IngestionJob>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM ingestion_jobs ORDER BY created_at DESC LIMIT ?1")?;
        let jobs = stmt
            .query_map(params![limit as i64], Self::row_to_job)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(jobs)
    }

    // ==================== File Operations ====================

    fn create_file(&self, file: &IngestionFile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Serialize conversion_reason to JSON
        let conversion_reason_json = file
            .conversion_reason
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());

        conn.execute(
            r#"
            INSERT INTO ingestion_files (
                id, job_id, filename, file_size_bytes, temp_file_path,
                duration_ms, codec, bitrate, sample_rate,
                tag_artist, tag_album, tag_title, tag_track_num, tag_track_total, tag_disc_num, tag_year,
                matched_track_id, match_confidence, output_file_path, converted, error_message,
                conversion_reason, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
            )
            "#,
            params![
                file.id,
                file.job_id,
                file.filename,
                file.file_size_bytes,
                file.temp_file_path,
                file.duration_ms,
                file.codec,
                file.bitrate,
                file.sample_rate,
                file.tag_artist,
                file.tag_album,
                file.tag_title,
                file.tag_track_num,
                file.tag_track_total,
                file.tag_disc_num,
                file.tag_year,
                file.matched_track_id,
                file.match_confidence,
                file.output_file_path,
                file.converted as i32,
                file.error_message,
                conversion_reason_json,
                file.created_at,
            ],
        )?;
        Ok(())
    }

    fn get_file(&self, id: &str) -> Result<Option<IngestionFile>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT * FROM ingestion_files WHERE id = ?1",
                params![id],
                Self::row_to_file,
            )
            .optional()?;
        Ok(result)
    }

    fn update_file(&self, file: &IngestionFile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Serialize conversion_reason to JSON
        let conversion_reason_json = file
            .conversion_reason
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());

        conn.execute(
            r#"
            UPDATE ingestion_files SET
                duration_ms = ?2, codec = ?3, bitrate = ?4, sample_rate = ?5,
                tag_artist = ?6, tag_album = ?7, tag_title = ?8, tag_track_num = ?9,
                tag_track_total = ?10, tag_disc_num = ?11, tag_year = ?12,
                matched_track_id = ?13, match_confidence = ?14,
                output_file_path = ?15, converted = ?16, error_message = ?17,
                conversion_reason = ?18
            WHERE id = ?1
            "#,
            params![
                file.id,
                file.duration_ms,
                file.codec,
                file.bitrate,
                file.sample_rate,
                file.tag_artist,
                file.tag_album,
                file.tag_title,
                file.tag_track_num,
                file.tag_track_total,
                file.tag_disc_num,
                file.tag_year,
                file.matched_track_id,
                file.match_confidence,
                file.output_file_path,
                file.converted as i32,
                file.error_message,
                conversion_reason_json,
            ],
        )?;
        Ok(())
    }

    fn get_files_for_job(&self, job_id: &str) -> Result<Vec<IngestionFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM ingestion_files WHERE job_id = ?1 ORDER BY tag_track_num ASC NULLS LAST, filename ASC",
        )?;
        let files = stmt
            .query_map(params![job_id], Self::row_to_file)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(files)
    }

    fn delete_files_for_job(&self, job_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM ingestion_files WHERE job_id = ?1",
            params![job_id],
        )?;
        Ok(())
    }

    // ==================== Reasoning Log ====================

    fn log_reasoning_step(&self, job_id: &str, step: &ReasoningStep) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let metadata = step
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        conn.execute(
            r#"
            INSERT INTO ingestion_reasoning_log (
                job_id, step_number, timestamp, step_type, content, metadata, duration_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                job_id,
                step.step_number,
                step.timestamp,
                format!("{:?}", step.step_type).to_lowercase(),
                step.content,
                metadata,
                step.duration_ms,
            ],
        )?;
        Ok(())
    }

    fn get_reasoning_steps(&self, job_id: &str) -> Result<Vec<ReasoningStep>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, step_number, timestamp, step_type, content, metadata, duration_ms
            FROM ingestion_reasoning_log
            WHERE job_id = ?1
            ORDER BY step_number ASC
            "#,
        )?;

        let steps = stmt
            .query_map(params![job_id], |row| {
                use crate::agent::reasoning::ReasoningStepType;
                let step_type_str: String = row.get("step_type")?;
                let step_type = match step_type_str.as_str() {
                    "context" => ReasoningStepType::Context,
                    "thought" => ReasoningStepType::Thought,
                    "toolcall" | "tool_call" => ReasoningStepType::ToolCall,
                    "toolresult" | "tool_result" => ReasoningStepType::ToolResult,
                    "decision" => ReasoningStepType::Decision,
                    "reviewquestion" | "review_question" => ReasoningStepType::ReviewQuestion,
                    "reviewanswer" | "review_answer" => ReasoningStepType::ReviewAnswer,
                    "action" => ReasoningStepType::Action,
                    _ => ReasoningStepType::Error,
                };

                let metadata_str: Option<String> = row.get("metadata")?;
                let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

                Ok(ReasoningStep {
                    id: row.get::<_, i64>("id")?.to_string(),
                    step_number: row.get("step_number")?,
                    timestamp: row.get("timestamp")?,
                    step_type,
                    content: row.get("content")?,
                    metadata,
                    duration_ms: row.get("duration_ms")?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(steps)
    }

    // ==================== Review Queue ====================

    fn create_review_item(&self, job_id: &str, question: &str, options: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO ingestion_review_queue (job_id, question, options, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                job_id,
                question,
                options,
                chrono::Utc::now().timestamp_millis(),
            ],
        )?;
        Ok(())
    }

    fn get_pending_reviews(&self, limit: usize) -> Result<Vec<ReviewQueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, job_id, question, options, created_at, resolved_at, resolved_by_user_id, selected_option
            FROM ingestion_review_queue
            WHERE resolved_at IS NULL
            ORDER BY created_at ASC
            LIMIT ?1
            "#,
        )?;

        let items = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ReviewQueueItem {
                    id: row.get("id")?,
                    job_id: row.get("job_id")?,
                    question: row.get("question")?,
                    options: row.get("options")?,
                    created_at: row.get("created_at")?,
                    resolved_at: row.get("resolved_at")?,
                    resolved_by_user_id: row.get("resolved_by_user_id")?,
                    selected_option: row.get("selected_option")?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn resolve_review(&self, job_id: &str, user_id: &str, selected_option: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE ingestion_review_queue
            SET resolved_at = ?1, resolved_by_user_id = ?2, selected_option = ?3
            WHERE job_id = ?4 AND resolved_at IS NULL
            "#,
            params![
                chrono::Utc::now().timestamp_millis(),
                user_id,
                selected_option,
                job_id,
            ],
        )?;
        Ok(())
    }

    fn get_review_item(&self, job_id: &str) -> Result<Option<ReviewQueueItem>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                r#"
                SELECT id, job_id, question, options, created_at, resolved_at, resolved_by_user_id, selected_option
                FROM ingestion_review_queue
                WHERE job_id = ?1
                "#,
                params![job_id],
                |row| {
                    Ok(ReviewQueueItem {
                        id: row.get("id")?,
                        job_id: row.get("job_id")?,
                        question: row.get("question")?,
                        options: row.get("options")?,
                        created_at: row.get("created_at")?,
                        resolved_at: row.get("resolved_at")?,
                        resolved_by_user_id: row.get("resolved_by_user_id")?,
                        selected_option: row.get("selected_option")?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::reasoning::ReasoningStepType;

    #[test]
    fn test_create_and_get_job() {
        let store = SqliteIngestionStore::in_memory().unwrap();
        let job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 12);

        store.create_job(&job).unwrap();
        let retrieved = store.get_job("job1").unwrap().unwrap();

        assert_eq!(retrieved.id, "job1");
        assert_eq!(retrieved.user_id, "user1");
        assert_eq!(retrieved.file_count, 12);
        assert_eq!(retrieved.status, IngestionJobStatus::Pending);
    }

    #[test]
    fn test_update_job() {
        let store = SqliteIngestionStore::in_memory().unwrap();
        let mut job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 12);
        store.create_job(&job).unwrap();

        job.status = IngestionJobStatus::IdentifyingAlbum;
        job.detected_artist = Some("The Beatles".to_string());
        job.detected_album = Some("Abbey Road".to_string());
        store.update_job(&job).unwrap();

        let retrieved = store.get_job("job1").unwrap().unwrap();
        assert_eq!(retrieved.status, IngestionJobStatus::IdentifyingAlbum);
        assert_eq!(retrieved.detected_artist, Some("The Beatles".to_string()));
    }

    #[test]
    fn test_file_crud() {
        let store = SqliteIngestionStore::in_memory().unwrap();
        let job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 2);
        store.create_job(&job).unwrap();

        let mut file1 = IngestionFile::new(
            "file1",
            "job1",
            "01 - Come Together.mp3",
            5000000,
            "/tmp/job1/01.mp3",
        );
        file1.tag_track_num = Some(1);
        file1.tag_title = Some("Come Together".to_string());
        store.create_file(&file1).unwrap();

        let mut file2 = IngestionFile::new(
            "file2",
            "job1",
            "02 - Something.mp3",
            4000000,
            "/tmp/job1/02.mp3",
        );
        file2.tag_track_num = Some(2);
        file2.tag_title = Some("Something".to_string());
        store.create_file(&file2).unwrap();

        let files = store.get_files_for_job("job1").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].tag_track_num, Some(1)); // Ordered by track number
        assert_eq!(files[1].tag_track_num, Some(2));

        // Update file
        let mut file = store.get_file("file1").unwrap().unwrap();
        file.matched_track_id = Some("track123".to_string());
        file.converted = true;
        store.update_file(&file).unwrap();

        let updated = store.get_file("file1").unwrap().unwrap();
        assert_eq!(updated.matched_track_id, Some("track123".to_string()));
        assert!(updated.converted);
    }

    #[test]
    fn test_list_jobs_by_user() {
        let store = SqliteIngestionStore::in_memory().unwrap();

        for i in 0..5 {
            let job = IngestionJob::new(format!("job{}", i), "user1", "album.zip", 1024000, 10);
            store.create_job(&job).unwrap();
        }

        let jobs = store.list_jobs_by_user("user1", 10).unwrap();
        assert_eq!(jobs.len(), 5);
    }

    #[test]
    fn test_reasoning_log() {
        let store = SqliteIngestionStore::in_memory().unwrap();
        let job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 10);
        store.create_job(&job).unwrap();

        let step = ReasoningStep::new(0, ReasoningStepType::Context, "Analyzing album upload");
        store.log_reasoning_step("job1", &step).unwrap();

        let steps = store.get_reasoning_steps("job1").unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].content, "Analyzing album upload");
    }

    #[test]
    fn test_review_queue() {
        let store = SqliteIngestionStore::in_memory().unwrap();
        let job = IngestionJob::new("job1", "user1", "album.zip", 1024000, 10);
        store.create_job(&job).unwrap();

        store
            .create_review_item(
                "job1",
                "Which album is this?",
                r#"[{"id":"album:abc","label":"Abbey Road"}]"#,
            )
            .unwrap();

        let pending = store.get_pending_reviews(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].job_id, "job1");

        store.resolve_review("job1", "admin", "album:abc").unwrap();

        let pending = store.get_pending_reviews(10).unwrap();
        assert_eq!(pending.len(), 0);

        let item = store.get_review_item("job1").unwrap().unwrap();
        assert!(item.resolved_at.is_some());
        assert_eq!(item.selected_option, Some("album:abc".to_string()));
    }
}
