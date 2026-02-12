//! Database schema for the ingestion feature.
//!
//! Album-first schema design:
//! - ingestion_jobs: One record per album upload
//! - ingestion_files: One record per audio file within a job
//! - ingestion_reasoning_log: Agent reasoning steps
//! - ingestion_review_queue: Human review items

/// SQL schema for the ingestion database (version 4 - fingerprint matching).
pub const INGESTION_SCHEMA_SQL: &str = r#"
-- Album-level job tracking
CREATE TABLE IF NOT EXISTS ingestion_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    user_id TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    total_size_bytes INTEGER NOT NULL,
    file_count INTEGER NOT NULL DEFAULT 0,

    -- Context
    context_type TEXT,
    context_id TEXT,

    -- Upload session and type (for collection uploads)
    upload_session_id TEXT,
    upload_type TEXT,

    -- Detected metadata (from embedded tags)
    detected_artist TEXT,
    detected_album TEXT,
    detected_year INTEGER,

    -- Album match result
    matched_album_id TEXT,
    match_confidence REAL,
    match_source TEXT,

    -- Fingerprint match details
    ticket_type TEXT,
    match_score REAL,
    match_delta_ms INTEGER,

    -- Stats
    tracks_matched INTEGER NOT NULL DEFAULT 0,
    tracks_converted INTEGER NOT NULL DEFAULT 0,

    -- Error handling
    error_message TEXT,

    -- Timestamps (Unix milliseconds)
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,

    -- Workflow state (JSON for resumable workflows)
    workflow_state TEXT
);

-- Individual audio files within a job
CREATE TABLE IF NOT EXISTS ingestion_files (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    temp_file_path TEXT NOT NULL,

    -- Audio metadata (from ffprobe)
    duration_ms INTEGER,
    codec TEXT,
    bitrate INTEGER,
    sample_rate INTEGER,

    -- Embedded tags (from ID3/Vorbis comments)
    tag_artist TEXT,
    tag_album TEXT,
    tag_title TEXT,
    tag_track_num INTEGER,
    tag_track_total INTEGER,
    tag_disc_num INTEGER,
    tag_year INTEGER,

    -- Track match result
    matched_track_id TEXT,
    match_confidence REAL,

    -- Output
    output_file_path TEXT,
    converted INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,

    -- Conversion decision (JSON serialized ConversionReason)
    conversion_reason TEXT,

    -- Timestamps
    created_at INTEGER NOT NULL,

    FOREIGN KEY (job_id) REFERENCES ingestion_jobs(id) ON DELETE CASCADE
);

-- Step-by-step reasoning log
CREATE TABLE IF NOT EXISTS ingestion_reasoning_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    step_number INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    step_type TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT,
    duration_ms INTEGER,
    FOREIGN KEY (job_id) REFERENCES ingestion_jobs(id) ON DELETE CASCADE
);

-- Human review queue
CREATE TABLE IF NOT EXISTS ingestion_review_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL UNIQUE,
    question TEXT NOT NULL,
    options TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolved_by_user_id TEXT,
    selected_option TEXT,
    FOREIGN KEY (job_id) REFERENCES ingestion_jobs(id) ON DELETE CASCADE
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_ingestion_jobs_status ON ingestion_jobs(status);
CREATE INDEX IF NOT EXISTS idx_ingestion_jobs_user ON ingestion_jobs(user_id);
CREATE INDEX IF NOT EXISTS idx_ingestion_files_job ON ingestion_files(job_id);
CREATE INDEX IF NOT EXISTS idx_ingestion_reasoning_job ON ingestion_reasoning_log(job_id);
CREATE INDEX IF NOT EXISTS idx_ingestion_review_pending ON ingestion_review_queue(resolved_at) WHERE resolved_at IS NULL;
"#;

/// Current schema version.
pub const INGESTION_SCHEMA_VERSION: i32 = 4;

/// Migration from version 2 to 3: Add conversion_reason column.
pub fn migrate_v2_to_v3(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    // Check if conversion_reason column already exists
    let column_exists: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_files') WHERE name='conversion_reason'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !column_exists {
        conn.execute(
            "ALTER TABLE ingestion_files ADD COLUMN conversion_reason TEXT",
            [],
        )?;
    }

    Ok(())
}

/// Migration from version 3 to 4: Add fingerprint matching columns.
pub fn migrate_v3_to_v4(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    // Add upload_session_id column
    let has_session_id: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_jobs') WHERE name='upload_session_id'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_session_id {
        conn.execute(
            "ALTER TABLE ingestion_jobs ADD COLUMN upload_session_id TEXT",
            [],
        )?;
    }

    // Add upload_type column
    let has_upload_type: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_jobs') WHERE name='upload_type'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_upload_type {
        conn.execute("ALTER TABLE ingestion_jobs ADD COLUMN upload_type TEXT", [])?;
    }

    // Add ticket_type column
    let has_ticket_type: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_jobs') WHERE name='ticket_type'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_ticket_type {
        conn.execute("ALTER TABLE ingestion_jobs ADD COLUMN ticket_type TEXT", [])?;
    }

    // Add match_score column
    let has_match_score: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_jobs') WHERE name='match_score'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_match_score {
        conn.execute("ALTER TABLE ingestion_jobs ADD COLUMN match_score REAL", [])?;
    }

    // Add match_delta_ms column
    let has_match_delta: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('ingestion_jobs') WHERE name='match_delta_ms'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_match_delta {
        conn.execute(
            "ALTER TABLE ingestion_jobs ADD COLUMN match_delta_ms INTEGER",
            [],
        )?;
    }

    // Create index on upload_session_id for grouping collection jobs
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ingestion_jobs_session ON ingestion_jobs(upload_session_id)",
        [],
    )?;

    Ok(())
}
