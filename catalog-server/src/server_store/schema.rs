/// Simple schema definition for server.db that uses raw SQL.
/// This is simpler than the table-based VersionedSchema used elsewhere
/// since server.db has few tables and straightforward migrations.
pub struct ServerSchema {
    pub version: usize,
    pub up: &'static str,
}

pub const SERVER_VERSIONED_SCHEMAS: &[ServerSchema] = &[
    ServerSchema {
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
    ServerSchema {
        version: 2,
        up: r#"
            CREATE TABLE server_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        "#,
    },
    ServerSchema {
        version: 3,
        up: r#"
            CREATE TABLE job_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                duration_ms INTEGER,
                details TEXT,
                error TEXT
            );

            CREATE INDEX idx_job_audit_log_job_id ON job_audit_log(job_id);
            CREATE INDEX idx_job_audit_log_timestamp ON job_audit_log(timestamp DESC);
            CREATE INDEX idx_job_audit_log_event_type ON job_audit_log(event_type);
        "#,
    },
];
