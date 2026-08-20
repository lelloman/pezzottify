impl SqliteDownloadQueueStore {
    /// Create a new SqliteDownloadQueueStore.
    ///
    /// Opens an existing database or creates a new one with the current schema.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    pub fn new<P: AsRef<Path>>(
        db_path: P,
        db_registry: &crate::backup::DbRegistry,
    ) -> Result<Self> {
        let conn = if db_path.as_ref().exists() {
            Connection::open_with_flags(
                &db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        } else {
            let conn = Connection::open(&db_path)?;
            // Create all schema versions in order (each version adds tables)
            for schema in DOWNLOAD_QUEUE_VERSIONED_SCHEMAS {
                schema.create(&conn)?;
            }
            info!(
                "Created new download queue database at {:?}",
                db_path.as_ref()
            );
            conn
        };

        crate::sqlite_persistence::configure_connection(&conn)?;

        // Read the database version
        let db_version = conn
            .query_row("PRAGMA user_version;", [], |row| row.get::<usize, i64>(0))
            .context("Failed to read database version")?
            - BASE_DB_VERSION as i64;

        if db_version < 0 {
            bail!(
                "Download queue database version {} is too old, does not contain base db version {}",
                db_version,
                BASE_DB_VERSION
            );
        }
        let version = db_version as usize;

        let schema_count = DOWNLOAD_QUEUE_VERSIONED_SCHEMAS.len();
        if version >= schema_count {
            bail!(
                "Download queue database version {} is too new (max supported: {})",
                version,
                schema_count - 1
            );
        }

        // Validate schema matches expected structure
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
            .get(version)
            .context("Failed to get schema")?
            .validate(&conn)?;

        // Run migrations if needed
        Self::migrate_if_needed(&conn, version)?;

        db_registry.register(db_path.as_ref().to_path_buf(), &conn)?;

        Ok(SqliteDownloadQueueStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory store for testing.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        crate::sqlite_persistence::configure_connection(&conn)?;

        // Create all schema versions in order (each version adds tables)
        for schema in DOWNLOAD_QUEUE_VERSIONED_SCHEMAS {
            schema.create(&conn)?;
        }

        Ok(SqliteDownloadQueueStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run any pending migrations.
    fn migrate_if_needed(conn: &Connection, current_version: usize) -> Result<()> {
        let target_version = DOWNLOAD_QUEUE_VERSIONED_SCHEMAS.len() - 1;

        if current_version >= target_version {
            return Ok(());
        }

        info!(
            "Migrating download queue database from version {} to {}",
            current_version, target_version
        );

        for schema in DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
            .iter()
            .skip(current_version + 1)
        {
            if let Some(migration_fn) = schema.migration {
                info!(
                    "Running download queue migration to version {}",
                    schema.version
                );
                migration_fn(conn)?;
            }
        }

        // Update version
        conn.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + target_version),
            [],
        )?;

        Ok(())
    }

    /// Get a reference to the connection for internal use.
    #[allow(dead_code)]
    pub(crate) fn connection(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }

    /// Helper to convert a database row to a QueueItem.
    fn row_to_queue_item(row: &rusqlite::Row) -> rusqlite::Result<QueueItem> {
        Ok(QueueItem {
            id: row.get("id")?,
            parent_id: row.get("parent_id")?,
            status: QueueStatus::from_db_str(&row.get::<_, String>("status")?),
            priority: QueuePriority::from_i32(row.get("priority")?).unwrap_or(QueuePriority::User),
            content_type: DownloadContentType::from_str(&row.get::<_, String>("content_type")?)
                .unwrap_or(DownloadContentType::Album),
            content_id: row.get("content_id")?,
            content_name: row.get("content_name")?,
            artist_name: row.get("artist_name")?,
            request_source: RequestSource::from_str(&row.get::<_, String>("request_source")?)
                .unwrap_or(RequestSource::User),
            requested_by_user_id: row.get("requested_by_user_id")?,
            created_at: row.get("created_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            last_attempt_at: row.get("last_attempt_at")?,
            next_retry_at: row.get("next_retry_at")?,
            retry_count: row.get("retry_count")?,
            max_retries: row.get("max_retries")?,
            error_type: row
                .get::<_, Option<String>>("error_type")?
                .and_then(|s| DownloadErrorType::from_str(&s)),
            error_message: row.get("error_message")?,
            bytes_downloaded: row.get("bytes_downloaded")?,
            processing_duration_ms: row.get("processing_duration_ms")?,
        })
    }

    /// Get current timestamp in seconds.
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Get today's date as a string in YYYY-MM-DD format.
    fn today_date_string() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Convert to days since epoch, then back to date components
        let days = secs / 86400;
        let mut year = 1970i32;
        let mut remaining_days = days as i32;

        // Calculate year
        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        // Calculate month and day
        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_months = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1u32;
        for days_in_month in days_in_months.iter() {
            if remaining_days < *days_in_month {
                break;
            }
            remaining_days -= days_in_month;
            month += 1;
        }
        let day = (remaining_days + 1) as u32;

        format!("{:04}-{:02}-{:02}", year, month, day)
    }

    /// Get current hour bucket (timestamp truncated to hour).
    fn hour_bucket() -> i64 {
        let now = Self::now();
        // Truncate to hour (3600 seconds)
        (now / 3600) * 3600
    }

    /// Get start of current day as hour bucket.
    fn day_start_bucket() -> i64 {
        let now = Self::now();
        // Truncate to day (86400 seconds)
        (now / 86400) * 86400
    }

    /// Helper to convert a database row to an AuditLogEntry.
    fn row_to_audit_entry(row: &rusqlite::Row) -> rusqlite::Result<AuditLogEntry> {
        let event_type_str: String = row.get("event_type")?;
        let content_type_str: Option<String> = row.get("content_type")?;
        let request_source_str: Option<String> = row.get("request_source")?;
        let details_str: Option<String> = row.get("details")?;

        Ok(AuditLogEntry {
            id: row.get("id")?,
            timestamp: row.get("timestamp")?,
            event_type: AuditEventType::from_str(&event_type_str)
                .unwrap_or(AuditEventType::RequestCreated),
            queue_item_id: row.get("queue_item_id")?,
            content_type: content_type_str.and_then(|s| DownloadContentType::from_str(&s)),
            content_id: row.get("content_id")?,
            user_id: row.get("user_id")?,
            request_source: request_source_str.and_then(|s| RequestSource::from_str(&s)),
            details: details_str.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }
}
