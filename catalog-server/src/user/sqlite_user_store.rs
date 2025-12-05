use crate::server::metrics::record_db_query;
use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema, BASE_DB_VERSION,
    DEFAULT_TIMESTAMP,
};
use crate::user::user_models::{
    BandwidthSummary, BandwidthUsage, CategoryBandwidth, DailyListeningStats, ListeningEvent,
    ListeningSummary, TrackListeningStats, UserListeningHistoryEntry,
};
use crate::user::device::{Device, DeviceRegistration, DeviceType};
use crate::user::user_store::{UserBandwidthStore, UserListeningStore, UserSettingsStore};
use std::collections::HashMap;
use crate::user::*;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};
use tracing::{debug, info};

use super::auth::PezzottifyHasher;
use rand::{rng, Rng};
use rand_distr::Alphanumeric;

/// V 0
const USER_TABLE_V_0: Table = Table {
    name: "user",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!("handle", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[("idx_user_handle", "handle")],
};
const LIKED_CONTENT_TABLE_V_0: Table = Table {
    name: "liked_content",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            non_null = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("content_id", &SqlType::Text, non_null = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[],
};
const AUTH_TOKEN_TABLE_V_0: Table = Table {
    name: "auth_token",
    columns: &[
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("value", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!("last_used", &SqlType::Integer),
    ],
    unique_constraints: &[],
    indices: &[("idx_auth_token_value", "value")],
};
const USER_PASSWORD_CREDENTIALS_V_0: Table = Table {
    name: "user_password_credentials",
    columns: &[
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("salt", &SqlType::Text, non_null = true),
        sqlite_column!("hash", &SqlType::Text, non_null = true),
        sqlite_column!("hasher", &SqlType::Text, non_null = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!("last_tried", &SqlType::Integer),
        sqlite_column!("last_used", &SqlType::Integer),
    ],
    unique_constraints: &[],
    indices: &[],
};

/// V 1
const LIKED_CONTENT_TABLE_V_1: Table = Table {
    name: "liked_content",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            non_null = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("content_id", &SqlType::Text, non_null = true),
        sqlite_column!(
            "content_type",
            &SqlType::Integer,
            non_null = true,
            default_value = None
        ),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[],
};

/// V 2
const LIKED_CONTENT_TABLE_V_2: Table = Table {
    name: "liked_content",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            non_null = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("content_id", &SqlType::Text, non_null = true),
        sqlite_column!(
            "content_type",
            &SqlType::Integer,
            non_null = true,
            default_value = None
        ),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[&["user_id", "content_id"]],
    indices: &[],
};

/// V 3
const USER_PLAYLIST_TABLE_V_3: Table = Table {
    name: "user_playlist",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Text,
            is_primary_key = true,
            non_null = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("name", &SqlType::Text),
        sqlite_column!(
            "creator_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[],
};
const USER_PLAYLIST_TRACKS_TABLE_V_3: Table = Table {
    name: "user_playlist_tracks",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!("track_id", &SqlType::Text, non_null = true),
        sqlite_column!(
            "playlist_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user_playlist",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("position", &SqlType::Integer, non_null = true),
    ],
    unique_constraints: &[],
    indices: &[],
};

/// V 4
const USER_ROLE_TABLE_V_4: Table = Table {
    name: "user_role",
    columns: &[
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("role", &SqlType::Text, non_null = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[&["user_id", "role"]],
    indices: &[("idx_user_role_user_id", "user_id")],
};
const USER_EXTRA_PERMISSION_TABLE_V_4: Table = Table {
    name: "user_extra_permission",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("permission", &SqlType::Integer, non_null = true),
        sqlite_column!("start_time", &SqlType::Integer, non_null = true),
        sqlite_column!("end_time", &SqlType::Integer),
        sqlite_column!("countdown", &SqlType::Integer),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[("idx_user_extra_permission_user_id", "user_id")],
};

/// V 5
/// Bandwidth usage tracking table - stores aggregated bandwidth data per user per day per endpoint category
const BANDWIDTH_USAGE_TABLE_V_5: Table = Table {
    name: "bandwidth_usage",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        // Date stored as YYYYMMDD integer for easy grouping and querying
        sqlite_column!("date", &SqlType::Integer, non_null = true),
        // Endpoint category: "stream", "image", "catalog", "search", "auth", "user", "admin", "other"
        sqlite_column!("endpoint_category", &SqlType::Text, non_null = true),
        // Total bytes sent in responses
        sqlite_column!("bytes_sent", &SqlType::Integer, non_null = true),
        // Total number of requests
        sqlite_column!("request_count", &SqlType::Integer, non_null = true),
        sqlite_column!(
            "updated",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    // Unique constraint ensures one row per user per day per endpoint category
    unique_constraints: &[&["user_id", "date", "endpoint_category"]],
    indices: &[
        ("idx_bandwidth_usage_user_id", "user_id"),
        ("idx_bandwidth_usage_date", "date"),
    ],
};

/// V 6
/// Listening events table - stores individual playback events for analytics
const LISTENING_EVENTS_TABLE_V_6: Table = Table {
    name: "listening_events",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        // Track identifier (e.g., "tra_xxxxx")
        sqlite_column!("track_id", &SqlType::Text, non_null = true),
        // Client-generated UUID for deduplication (supports offline queue retry)
        sqlite_column!("session_id", &SqlType::Text, is_unique = true),
        // Unix timestamp when playback started
        sqlite_column!("started_at", &SqlType::Integer, non_null = true),
        // Unix timestamp when playback ended
        sqlite_column!("ended_at", &SqlType::Integer),
        // Actual listening time in seconds (excluding pauses)
        sqlite_column!("duration_seconds", &SqlType::Integer, non_null = true),
        // Total track duration in seconds (for completion calculation)
        sqlite_column!("track_duration_seconds", &SqlType::Integer, non_null = true),
        // 1 if >90% of track was played
        sqlite_column!(
            "completed",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        // Number of seek operations during playback
        sqlite_column!("seek_count", &SqlType::Integer, default_value = Some("0")),
        // Number of pause/resume cycles
        sqlite_column!("pause_count", &SqlType::Integer, default_value = Some("0")),
        // Context: "album", "playlist", "track", "search"
        sqlite_column!("playback_context", &SqlType::Text),
        // Client type: "web", "android", "ios"
        sqlite_column!("client_type", &SqlType::Text),
        // Date in YYYYMMDD format for efficient date-range queries
        sqlite_column!("date", &SqlType::Integer, non_null = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_listening_events_user_id", "user_id"),
        ("idx_listening_events_track_id", "track_id"),
        ("idx_listening_events_date", "date"),
        ("idx_listening_events_session_id", "session_id"),
    ],
};

/// V 7
/// User settings table - key-value store for user preferences synced with server
const USER_SETTINGS_TABLE_V_7: Table = Table {
    name: "user_settings",
    columns: &[
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("setting_key", &SqlType::Text, non_null = true),
        sqlite_column!("setting_value", &SqlType::Text),
        sqlite_column!(
            "updated",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[&["user_id", "setting_key"]],
    indices: &[("idx_user_settings_user_id", "user_id")],
};

/// V 8
/// Device table - tracks client devices for session management
const DEVICE_TABLE_V_8: Table = Table {
    name: "device",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true, is_unique = true),
        sqlite_column!("device_uuid", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::SetNull,
            })
        ),
        sqlite_column!("device_type", &SqlType::Text, non_null = true),
        sqlite_column!("device_name", &SqlType::Text),
        sqlite_column!("os_info", &SqlType::Text),
        sqlite_column!(
            "first_seen",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!(
            "last_seen",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_device_user", "user_id"),
        ("idx_device_uuid", "device_uuid"),
    ],
};

/// V 8
/// Auth token table with device_id foreign key
const AUTH_TOKEN_TABLE_V_8: Table = Table {
    name: "auth_token",
    columns: &[
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("value", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!("last_used", &SqlType::Integer),
        sqlite_column!(
            "device_id",
            &SqlType::Integer,
            foreign_key = Some(&ForeignKey {
                foreign_table: "device",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_auth_token_value", "value"),
        ("idx_auth_token_device", "device_id"),
    ],
};

pub const VERSIONED_SCHEMAS: &[VersionedSchema] = &[
    VersionedSchema {
        version: 0,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_0,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
        ],
        migration: None,
    },
    VersionedSchema {
        version: 1,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_1,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
        ],
        migration: Some(|conn: &Connection| {
            conn.execute(
                "ALTER TABLE liked_content ADD COLUMN content_type INTEGER NOT NULL DEFAULT 1000",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 2,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
        ],
        migration: Some(|conn: &Connection| {
            // Rename liked_content to liked_content_backup
            conn.execute(
                "ALTER TABLE liked_content RENAME TO liked_content_backup;",
                [],
            )?;

            // Create the new liked_content table
            LIKED_CONTENT_TABLE_V_2.create(conn)?;

            // Migrate data from liked_content_backup to liked_content
            let mut stmt = conn.prepare(
                "SELECT id, user_id, content_id, content_type, created FROM liked_content_backup;",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<usize, i64>(0)?,
                    row.get::<usize, String>(1)?,
                    row.get::<usize, String>(2)?,
                    row.get::<usize, i32>(3)?,
                    row.get::<usize, i64>(4)?,
                ))
            })?;

            for row in rows {
                let (id, user_id, content_id, content_type, created) = row?;
                let _ = conn.execute(
                    "INSERT INTO liked_content (id, user_id, content_id, content_type, created) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, user_id, content_id, content_type, created],
                );
            }

            // Drop the backup table
            conn.execute("DROP TABLE liked_content_backup;", [])?;

            Ok(())
        }),
    },
    VersionedSchema {
        version: 3,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
        ],
        migration: Some(|conn: &Connection| {
            USER_PLAYLIST_TABLE_V_3.create(&conn)?;
            USER_PLAYLIST_TRACKS_TABLE_V_3.create(&conn)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 4,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
        ],
        migration: Some(|conn: &Connection| {
            USER_ROLE_TABLE_V_4.create(&conn)?;
            USER_EXTRA_PERMISSION_TABLE_V_4.create(&conn)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 5,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
            BANDWIDTH_USAGE_TABLE_V_5,
        ],
        migration: Some(|conn: &Connection| {
            BANDWIDTH_USAGE_TABLE_V_5.create(&conn)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 6,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
            BANDWIDTH_USAGE_TABLE_V_5,
            LISTENING_EVENTS_TABLE_V_6,
        ],
        migration: Some(|conn: &Connection| {
            LISTENING_EVENTS_TABLE_V_6.create(&conn)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 7,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
            BANDWIDTH_USAGE_TABLE_V_5,
            LISTENING_EVENTS_TABLE_V_6,
            USER_SETTINGS_TABLE_V_7,
        ],
        migration: Some(|conn: &Connection| {
            USER_SETTINGS_TABLE_V_7.create(&conn)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 8,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_8,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
            BANDWIDTH_USAGE_TABLE_V_5,
            LISTENING_EVENTS_TABLE_V_6,
            USER_SETTINGS_TABLE_V_7,
            DEVICE_TABLE_V_8,
        ],
        migration: Some(|conn: &Connection| {
            // Step 1: Create device table first (auth_token will reference it)
            DEVICE_TABLE_V_8.create(&conn)?;

            // Step 2: Delete all existing tokens (no real users yet, per plan)
            conn.execute("DELETE FROM auth_token", [])?;

            // Step 3: Recreate auth_token with device_id column
            // SQLite doesn't support ADD COLUMN with NOT NULL and FK well
            conn.execute("DROP TABLE auth_token", [])?;
            AUTH_TOKEN_TABLE_V_8.create(&conn)?;

            Ok(())
        }),
    },
];

/// A random A-z0-9 string
fn random_string(len: usize) -> String {
    let bytes = rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .collect::<Vec<u8>>();
    String::from_utf8_lossy(&bytes).to_string()
}

#[derive(Clone)]
pub struct SqliteUserStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteUserStore {
    pub fn new<T: AsRef<Path>>(db_path: T) -> Result<Self> {
        let mut conn = if db_path.as_ref().exists() {
            Connection::open_with_flags(
                db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        } else {
            let conn = Connection::open(db_path)?;
            VERSIONED_SCHEMAS.last().unwrap().create(&conn)?;
            conn
        };

        // Read the database version
        let db_version = conn
            .query_row("PRAGMA user_version;", [], |row| row.get::<usize, i64>(0))
            .context("Failed to read database version")?
            - BASE_DB_VERSION as i64;

        if db_version < 0 {
            bail!(
                "Database version {} is too old, does not contain base db version {}",
                db_version,
                BASE_DB_VERSION
            );
        }
        let version = db_version as usize;

        if db_version >= VERSIONED_SCHEMAS.len() as i64 {
            bail!("Database version {} is too new", db_version);
        } else {
            VERSIONED_SCHEMAS
                .get(version)
                .context("Failed to get schema")?
                .validate(&conn)?;
        }

        Self::migrate_if_needed(&mut conn, version)?;

        Ok(SqliteUserStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn infer_path() -> Option<PathBuf> {
        let db_data_dir = PathBuf::from("/data/db/user.db");
        if db_data_dir.exists() {
            return Some(db_data_dir);
        }

        let mut current_dir = std::env::current_dir().ok()?;
        loop {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    if path.is_file() {
                        if let Some(s) = path.file_name() {
                            if s.to_string_lossy() == "user.db" {
                                return Some(s.into());
                            }
                        }
                    }
                }
            }
            if let Some(parent) = current_dir.parent() {
                current_dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        None
    }

    fn migrate_if_needed(conn: &mut Connection, version: usize) -> Result<()> {
        let tx = conn.transaction()?;
        let mut latest_from = version;
        for schema in VERSIONED_SCHEMAS.iter().skip(version + 1) {
            if let Some(migration_fn) = schema.migration {
                info!(
                    "Migrating db from version {} to {}",
                    latest_from, schema.version
                );
                migration_fn(&tx)?;
                latest_from = schema.version;
            }
        }
        tx.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + latest_from),
            [],
        )?;

        tx.commit()?;
        Ok(())
    }
}

impl UserStore for SqliteUserStore {
    fn create_user(&self, user_handle: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO user (handle) VALUES (?1)",
            params![user_handle],
        )
        .with_context(|| format!("Failed to create user {}", user_handle))?;

        Ok(conn.last_insert_rowid() as usize)
    }

    fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, name FROM {} WHERE user_id = ?1",
            USER_PLAYLIST_TABLE_V_3.name
        ))?;
        let playlists = stmt
            .query_map(params![user_id], |row| Ok(row.get(0)?))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(playlists)
    }

    fn get_user_handle(&self, user_id: usize) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT handle FROM {} WHERE id = ?1",
                USER_TABLE_V_0.name
            ))?;
        match stmt.query_row(params![user_id], |row| row.get(0)) {
            Ok(handle) => Ok(Some(handle)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_all_user_handles(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT handle FROM {}", USER_TABLE_V_0.name))?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(rows)
    }

    fn get_user_id(&self, user_handle: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM {} WHERE handle = ?1",
                USER_TABLE_V_0.name
            ))?;
        match stmt.query_row(params![user_handle], |row| row.get(0)) {
            Ok(id) => {
                let id: i32 = id;
                Ok(Some(id as usize))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Result<Option<bool>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT COUNT(*) FROM {} WHERE user_id = ?1 AND content_id = ?2",
                LIKED_CONTENT_TABLE_V_2.name
            ))?;
        let count: i32 = stmt
            .query_row(params![user_id, content_id], |row| row.get(0))?;

        Ok(Some(count > 0))
    }

    fn set_user_liked_content(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if liked {
            conn.execute(
                &format!(
                    "INSERT OR IGNORE INTO {} (user_id, content_id, content_type) VALUES (?1, ?2, ?3)",
                    LIKED_CONTENT_TABLE_V_2.name
                ),
                params![user_id, content_id, content_type.to_int()],
            )?;
        } else {
            conn.execute(
                &format!(
                    "DELETE FROM {} WHERE user_id = ?1 AND content_id = ?2",
                    LIKED_CONTENT_TABLE_V_2.name
                ),
                params![user_id, content_id],
            )?;
        }

        Ok(())
    }

    fn get_user_liked_content(
        &self,
        user_id: usize,
        content_type: LikedContentType,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT content_id FROM {} WHERE user_id = ?1 AND content_type = ?2",
                LIKED_CONTENT_TABLE_V_2.name
            ))
            .ok()
            .unwrap();
        Ok(stmt
            .query_map(params![user_id, content_type.to_int()], |row| row.get(0))
            .ok()
            .unwrap()
            .collect::<Result<Vec<String>, _>>()?)
    }

    fn create_user_playlist(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_user_id: usize,
        track_ids: Vec<String>,
    ) -> Result<String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Generate a random 16 A-z0-9 string that's not already a playlist id
        let mut playlist_id = random_string(16);
        while tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE id = ?1",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id],
            |row| row.get::<usize, i64>(0),
        )? > 0
        {
            playlist_id = random_string(16);
        }

        tx.execute(
            &format!(
                "INSERT INTO {} (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![&playlist_id, user_id, playlist_name, creator_user_id],
        )
        .context("Could not create playlist")?;

        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                &format!(
                    "INSERT INTO {} (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                    USER_PLAYLIST_TRACKS_TABLE_V_3.name
                ),
                params![playlist_id, track_id, position as i32],
            )?;
        }

        tx.commit()?;
        Ok(playlist_id)
    }

    fn update_user_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let playlist_user_id = tx.query_row(
            &format!(
                "SELECT user_id FROM {} WHERE id = ?1",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id],
            |row| row.get::<usize, usize>(0),
        )?;
        debug!("update_user_playlist({playlist_id}) found user_id: {playlist_user_id}",);
        if user_id != playlist_user_id {
            bail!("User does not own the playlist");
        }

        if let Some(playlist_name) = playlist_name {
            debug!("update_user_playlist({playlist_id}) updating name to {playlist_name}",);
            tx.execute(
                &format!(
                    "UPDATE {} SET name = ?1 WHERE id = ?2",
                    USER_PLAYLIST_TABLE_V_3.name
                ),
                params![playlist_name, playlist_id],
            )?;
        }

        if let Some(track_ids) = track_ids {
            debug!("update_user_playlist({playlist_id}) updating tracks",);
            tx.execute(
                &format!(
                    "DELETE FROM {} WHERE playlist_id = ?1",
                    USER_PLAYLIST_TRACKS_TABLE_V_3.name
                ),
                params![playlist_id],
            )?;

            for (position, track_id) in track_ids.iter().enumerate() {
                tx.execute(
                    &format!(
                        "INSERT INTO {} (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                        USER_PLAYLIST_TRACKS_TABLE_V_3.name
                    ),
                    params![playlist_id, track_id, position as i32],
                )?;
            }
        }
        debug!("update_user_playlist({playlist_id}) committing...",);
        tx.commit()?;
        Ok(())
    }

    fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1 AND user_id = ?2",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id, user_id],
        )?;
        Ok(())
    }

    fn get_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<UserPlaylist> {
        let conn = self.conn.lock().unwrap();

        debug!("get_user_playlist({playlist_id})");

        let creator_name = conn.query_row(
            &format!(
                "SELECT handle FROM {} WHERE id = (SELECT creator_id FROM {} WHERE id = ?1)",
                USER_TABLE_V_0.name, USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id],
            |row| row.get(0),
        )?;
        debug!("get_user_playlist({playlist_id}) found creator name: {creator_name}",);

        let mut stmt = conn.prepare(&format!(
            "SELECT id, name, created FROM {} WHERE id = ?1 AND user_id = ?2",
            USER_PLAYLIST_TABLE_V_3.name
        ))?;
        let mut playlist = stmt.query_row(params![playlist_id, user_id], |row| {
            Ok(UserPlaylist {
                id: row.get(0)?,
                user_id: user_id,
                creator: creator_name,
                name: row.get(1)?,
                created: system_time_from_column_result(row.get(2)?),
                tracks: vec![],
            })
        })?;

        debug!("get_user_playlist({playlist_id}) fetching tracks...",);
        let track_ids = conn
            .prepare(&format!(
                "SELECT track_id FROM {} WHERE playlist_id = ?1 ORDER BY position",
                USER_PLAYLIST_TRACKS_TABLE_V_3.name
            ))?
            .query_map(params![playlist_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        playlist.tracks = track_ids;
        Ok(playlist)
    }

    fn get_user_roles(&self, user_id: usize) -> Result<Vec<UserRole>> {
        debug!("get_user_roles: querying roles for user_id={}", user_id);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT role FROM {} WHERE user_id = ?1",
            USER_ROLE_TABLE_V_4.name
        ))?;
        let roles = stmt
            .query_map(params![user_id], |row| {
                let role_str: String = row.get(0)?;
                debug!("get_user_roles: found role string '{}' for user_id={}", role_str, user_id);
                Ok(role_str)
            })?
            .filter_map(|r| r.ok())
            .flat_map(|s| {
                s.split(',')
                    .map(|part| part.trim())
                    .filter_map(|part| UserRole::from_str(part))
                    .collect::<Vec<_>>()
            })
            .collect();
        debug!("get_user_roles: returning {:?} for user_id={}", roles, user_id);
        Ok(roles)
    }

    fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Try to get existing roles for this user
        let existing_roles: Option<String> = tx
            .query_row(
                &format!("SELECT role FROM {} WHERE user_id = ?1", USER_ROLE_TABLE_V_4.name),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse existing roles and check if this role is already present
            let mut roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(|s| UserRole::from_str(s))
                .collect();

            if !roles.contains(&role) {
                roles.push(role);
                let roles_str = roles
                    .iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!("UPDATE {} SET role = ?1 WHERE user_id = ?2", USER_ROLE_TABLE_V_4.name),
                    params![roles_str, user_id],
                )?;
            }
        } else {
            // No existing roles, insert new row
            tx.execute(
                &format!(
                    "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id, role.to_string()],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn remove_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get existing roles for this user
        let existing_roles: Option<String> = tx
            .query_row(
                &format!("SELECT role FROM {} WHERE user_id = ?1", USER_ROLE_TABLE_V_4.name),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse and filter out the role to remove
            let roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(|s| UserRole::from_str(s))
                .filter(|r| r != &role)
                .collect();

            if roles.is_empty() {
                // No roles left, delete the row
                tx.execute(
                    &format!("DELETE FROM {} WHERE user_id = ?1", USER_ROLE_TABLE_V_4.name),
                    params![user_id],
                )?;
            } else {
                // Update with remaining roles
                let roles_str = roles
                    .iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!("UPDATE {} SET role = ?1 WHERE user_id = ?2", USER_ROLE_TABLE_V_4.name),
                    params![roles_str, user_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn add_user_extra_permission(&self, user_id: usize, grant: PermissionGrant) -> Result<usize> {
        match grant {
            PermissionGrant::ByRole(_) => {
                bail!("Cannot add ByRole grant as extra permission");
            }
            PermissionGrant::Extra {
                start_time,
                end_time,
                permission,
                countdown,
            } => {
                let conn = self.conn.lock().unwrap();
                let start_time_secs = start_time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let end_time_secs = end_time
                    .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64);
                let countdown_i64 = countdown.map(|c| c as i64);

                conn.execute(
                    &format!(
                        "INSERT INTO {} (user_id, permission, start_time, end_time, countdown) VALUES (?1, ?2, ?3, ?4, ?5)",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![user_id, permission.to_int(), start_time_secs, end_time_secs, countdown_i64],
                )?;
                Ok(conn.last_insert_rowid() as usize)
            }
        }
    }

    fn remove_user_extra_permission(&self, permission_id: usize) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
        )?;
        Ok(())
    }

    fn decrement_permission_countdown(&self, permission_id: usize) -> Result<bool> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get current countdown
        let current_countdown: Option<i64> = tx.query_row(
            &format!(
                "SELECT countdown FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
            |row| row.get(0),
        )?;

        let result = match current_countdown {
            None => Ok(true), // No countdown, permission remains valid
            Some(count) if count <= 1 => {
                // Last use, delete the permission
                tx.execute(
                    &format!(
                        "DELETE FROM {} WHERE id = ?1",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![permission_id],
                )?;
                Ok(false)
            }
            Some(count) => {
                // Decrement the countdown
                tx.execute(
                    &format!(
                        "UPDATE {} SET countdown = ?1 WHERE id = ?2",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![count - 1, permission_id],
                )?;
                Ok(true)
            }
        };

        tx.commit()?;
        result
    }

    fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>> {
        use std::collections::HashSet;

        debug!("resolve_user_permissions: starting for user_id={}", user_id);
        let mut permissions = HashSet::new();

        // Add permissions from roles
        let roles = self.get_user_roles(user_id)?;
        debug!("resolve_user_permissions: user_id={} has roles: {:?}", user_id, roles);
        for role in &roles {
            let role_perms = role.permissions();
            debug!("resolve_user_permissions: adding {:?} permissions from role {:?}", role_perms.len(), role);
            for permission in role_perms {
                permissions.insert(*permission);
            }
        }

        // Add active extra permissions
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        debug!("resolve_user_permissions: checking extra permissions for user_id={} at timestamp={}", user_id, now);

        let mut stmt = conn.prepare(&format!(
            "SELECT permission FROM {} WHERE user_id = ?1 AND start_time <= ?2 AND (end_time IS NULL OR end_time >= ?2) AND (countdown IS NULL OR countdown > 0)",
            USER_EXTRA_PERMISSION_TABLE_V_4.name
        ))?;

        let extra_perms = stmt
            .query_map(params![user_id, now], |row| {
                let perm_int: i32 = row.get(0)?;
                Ok(perm_int)
            })?
            .filter_map(|r| r.ok().and_then(|i| Permission::from_int(i)))
            .collect::<Vec<_>>();

        debug!("resolve_user_permissions: found {} extra permissions for user_id={}", extra_perms.len(), user_id);
        for perm in &extra_perms {
            debug!("resolve_user_permissions: adding extra permission {:?}", perm);
            permissions.insert(*perm);
        }

        let final_permissions: Vec<Permission> = permissions.into_iter().collect();
        debug!("resolve_user_permissions: final permissions for user_id={}: {:?}", user_id, final_permissions);
        Ok(final_permissions)
    }
}

fn system_time_from_column_result(value: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(value as u64)
}

impl UserAuthTokenStore for SqliteUserStore {
    fn get_user_auth_token(&self, value: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM auth_token WHERE value = ?1")?;
        let result = match stmt.query_row(params![value.0], |row| {
            Ok(AuthToken {
                user_id: row.get(0)?,
                device_id: None, // Will be populated when device_id column is added to schema
                value: AuthTokenValue(row.get(1)?),
                created: system_time_from_column_result(row.get(2)?),
                last_used: row
                    .get::<usize, Option<i64>>(3)?
                    .map(|v| system_time_from_column_result(v)),
            })
        }) {
            Ok(token) => Ok(Some(token)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        };
        record_db_query("get_user_auth_token", start.elapsed());
        result
    }

    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get the token data before deleting
        let auth_token = match tx
            .prepare("SELECT * FROM auth_token WHERE value = ?1")
            .and_then(|mut stmt| {
                stmt.query_row(params![token.0], |row| {
                    Ok(AuthToken {
                        user_id: row.get(0)?,
                        device_id: None, // Will be populated when device_id column is added to schema
                        value: AuthTokenValue(row.get(1)?),
                        created: system_time_from_column_result(row.get(2)?),
                        last_used: row
                            .get::<usize, Option<i64>>(3)?
                            .map(|v| system_time_from_column_result(v)),
                    })
                })
            }) {
                Ok(token) => token,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

        // Delete the token
        tx.execute("DELETE FROM auth_token WHERE value = ?1", params![token.0])?;

        tx.commit()?;
        Ok(Some(auth_token))
    }

    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE auth_token SET last_used = ?1 WHERE value = ?2",
            params![now, token.0],
        )?;
        Ok(())
    }

    fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO auth_token (value, user_id) VALUES (?1, ?2)",
            params![token.value.0, token.user_id,],
        )?;
        record_db_query("add_user_auth_token", start.elapsed());
        Ok(())
    }

    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Result<Vec<AuthToken>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT * FROM auth_token WHERE user_id = (SELECT id FROM user WHERE handle = ?1)",
            )?;
        let rows = stmt
            .query_map(params![user_handle], |row| {
                Ok(AuthToken {
                    user_id: row.get(0)?,
                    device_id: None, // Will be populated when device_id column is added to schema
                    value: AuthTokenValue(row.get(1)?),
                    created: system_time_from_column_result(row.get(2)?),
                    last_used: row
                        .get::<usize, Option<i64>>(3)?
                        .map(|v| system_time_from_column_result(v)),
                })
            })?
            .collect::<Result<Vec<AuthToken>, _>>()?;

        Ok(rows)
    }

    fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff_secs = now - (unused_for_days * 24 * 60 * 60) as i64;

        // Delete tokens that have never been used and are older than the cutoff
        // OR have been used but the last use is older than the cutoff
        let deleted = conn.execute(
            "DELETE FROM auth_token WHERE (last_used IS NULL AND created < ?1) OR (last_used IS NOT NULL AND last_used < ?1)",
            params![cutoff_secs],
        )?;

        Ok(deleted)
    }
}

impl UserAuthCredentialsStore for SqliteUserStore {
    fn get_user_auth_credentials(&self, user_handle: &str) -> Result<Option<UserAuthCredentials>> {
        let start = Instant::now();
        let user_id = match self.get_user_id(user_handle)? {
            Some(id) => id,
            None => {
                record_db_query("get_user_auth_credentials", start.elapsed());
                return Ok(None);
            }
        };
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM user_password_credentials WHERE user_id = ?1")?;

        let password_credentials = match stmt
            .query_row(params![user_id], |row| {
                let hasher = match PezzottifyHasher::from_str(&row.get::<usize, String>(3)?) {
                    Ok(x) => x,
                    Err(_) => {
                        eprintln!("get_user_auth_credentials() -> Invalid hasher");
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                };
                let user_id: usize = row.get(0)?;
                let salt: String = row.get(1)?;
                let hash: String = row.get(2)?;
                let created = system_time_from_column_result(row.get(4).unwrap());
                Ok(UsernamePasswordCredentials {
                    user_id,
                    salt,
                    hash,
                    hasher,
                    created,
                    last_tried: row
                        .get::<usize, Option<i64>>(5)?
                        .map(|v| system_time_from_column_result(v)),
                    last_used: row
                        .get::<usize, Option<i64>>(6)?
                        .map(|v| system_time_from_column_result(v)),
                })
            }) {
                Ok(creds) => Some(creds),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e.into()),
            };

        record_db_query("get_user_auth_credentials", start.elapsed());
        Ok(Some(UserAuthCredentials {
            user_id,
            username_password: password_credentials,
            keys: vec![],
        }))
    }

    fn update_user_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let user_id = credentials.user_id;
        match credentials.username_password.as_ref() {
            Some(password_credentials) => {
                let updated = tx.execute(
                    "UPDATE user_password_credentials SET salt = ?1, hash = ?2, hasher = ?3 WHERE user_id = ?4",
                    params![
                        password_credentials.salt,
                        password_credentials.hash,
                        password_credentials.hasher.to_string(),
                        user_id
                    ],
                )?;
                if updated == 0 {
                    tx.execute(
                        "INSERT INTO user_password_credentials (salt, hash, hasher, user_id) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            password_credentials.salt,
                            password_credentials.hash,
                            password_credentials.hasher.to_string(),
                            user_id
                        ],
                    )?;
                }
            }
            None => {
                tx.execute(
                    "DELETE FROM user_password_credentials WHERE user_id = ?1",
                    params![user_id],
                )?;
            }
        };
        tx.commit()?;
        Ok(())
    }
}

impl UserBandwidthStore for SqliteUserStore {
    fn record_bandwidth_usage(
        &self,
        user_id: usize,
        date: u32,
        endpoint_category: &str,
        bytes_sent: u64,
        request_count: u64,
    ) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Use INSERT OR REPLACE to upsert - if the unique constraint (user_id, date, endpoint_category) exists,
        // we need to add to existing values, so we use a subquery
        conn.execute(
            &format!(
                "INSERT INTO {} (user_id, date, endpoint_category, bytes_sent, request_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(user_id, date, endpoint_category) DO UPDATE SET
                 bytes_sent = bytes_sent + excluded.bytes_sent,
                 request_count = request_count + excluded.request_count,
                 updated = (cast(strftime('%s','now') as int))",
                BANDWIDTH_USAGE_TABLE_V_5.name
            ),
            params![user_id, date, endpoint_category, bytes_sent as i64, request_count as i64],
        )?;

        record_db_query("record_bandwidth_usage", start.elapsed());
        Ok(())
    }

    fn get_user_bandwidth_usage(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT user_id, date, endpoint_category, bytes_sent, request_count
             FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY date DESC, endpoint_category",
            BANDWIDTH_USAGE_TABLE_V_5.name
        ))?;

        let records = stmt
            .query_map(params![user_id, start_date, end_date], |row| {
                Ok(BandwidthUsage {
                    user_id: row.get::<_, i64>(0)? as usize,
                    date: row.get::<_, i64>(1)? as u32,
                    endpoint_category: row.get(2)?,
                    bytes_sent: row.get::<_, i64>(3)? as u64,
                    request_count: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_bandwidth_usage", start.elapsed());
        Ok(records)
    }

    fn get_user_bandwidth_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        let records = self.get_user_bandwidth_usage(user_id, start_date, end_date)?;

        let mut summary = BandwidthSummary {
            user_id: Some(user_id),
            total_bytes_sent: 0,
            total_requests: 0,
            by_category: std::collections::HashMap::new(),
        };

        for record in records {
            summary.total_bytes_sent += record.bytes_sent;
            summary.total_requests += record.request_count;

            let cat_entry = summary
                .by_category
                .entry(record.endpoint_category)
                .or_insert(CategoryBandwidth {
                    bytes_sent: 0,
                    request_count: 0,
                });
            cat_entry.bytes_sent += record.bytes_sent;
            cat_entry.request_count += record.request_count;
        }

        Ok(summary)
    }

    fn get_all_bandwidth_usage(&self, start_date: u32, end_date: u32) -> Result<Vec<BandwidthUsage>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT user_id, date, endpoint_category, bytes_sent, request_count
             FROM {} WHERE date >= ?1 AND date <= ?2
             ORDER BY user_id, date DESC, endpoint_category",
            BANDWIDTH_USAGE_TABLE_V_5.name
        ))?;

        let records = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(BandwidthUsage {
                    user_id: row.get::<_, i64>(0)? as usize,
                    date: row.get::<_, i64>(1)? as u32,
                    endpoint_category: row.get(2)?,
                    bytes_sent: row.get::<_, i64>(3)? as u64,
                    request_count: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_all_bandwidth_usage", start.elapsed());
        Ok(records)
    }

    fn get_total_bandwidth_summary(&self, start_date: u32, end_date: u32) -> Result<BandwidthSummary> {
        let records = self.get_all_bandwidth_usage(start_date, end_date)?;

        let mut summary = BandwidthSummary {
            user_id: None,
            total_bytes_sent: 0,
            total_requests: 0,
            by_category: std::collections::HashMap::new(),
        };

        for record in records {
            summary.total_bytes_sent += record.bytes_sent;
            summary.total_requests += record.request_count;

            let cat_entry = summary
                .by_category
                .entry(record.endpoint_category)
                .or_insert(CategoryBandwidth {
                    bytes_sent: 0,
                    request_count: 0,
                });
            cat_entry.bytes_sent += record.bytes_sent;
            cat_entry.request_count += record.request_count;
        }

        Ok(summary)
    }

    fn prune_bandwidth_usage(&self, older_than_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Calculate the cutoff date in YYYYMMDD format
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_secs = now - (older_than_days as u64 * 24 * 60 * 60);

        // Convert to YYYYMMDD format
        let cutoff_date = {
            let datetime = chrono::DateTime::from_timestamp(cutoff_secs as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());
            datetime.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0)
        };

        let deleted = conn.execute(
            &format!(
                "DELETE FROM {} WHERE date < ?1",
                BANDWIDTH_USAGE_TABLE_V_5.name
            ),
            params![cutoff_date],
        )?;

        record_db_query("prune_bandwidth_usage", start.elapsed());
        Ok(deleted)
    }
}

impl UserListeningStore for SqliteUserStore {
    fn record_listening_event(&self, event: ListeningEvent) -> Result<(usize, bool)> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // If session_id is provided, use INSERT OR IGNORE for deduplication
        // If session_id already exists, the insert will be ignored (idempotent)
        let result = if let Some(ref session_id) = event.session_id {
            conn.execute(
                &format!(
                    "INSERT OR IGNORE INTO {} (user_id, track_id, session_id, started_at, ended_at,
                     duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
                     playback_context, client_type, date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    LISTENING_EVENTS_TABLE_V_6.name
                ),
                params![
                    event.user_id,
                    event.track_id,
                    session_id,
                    event.started_at as i64,
                    event.ended_at.map(|t| t as i64),
                    event.duration_seconds as i64,
                    event.track_duration_seconds as i64,
                    if event.completed { 1 } else { 0 },
                    event.seek_count as i64,
                    event.pause_count as i64,
                    event.playback_context,
                    event.client_type,
                    event.date as i64,
                ],
            )?
        } else {
            // No session_id, always insert
            conn.execute(
                &format!(
                    "INSERT INTO {} (user_id, track_id, session_id, started_at, ended_at,
                     duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
                     playback_context, client_type, date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    LISTENING_EVENTS_TABLE_V_6.name
                ),
                params![
                    event.user_id,
                    event.track_id,
                    event.session_id,
                    event.started_at as i64,
                    event.ended_at.map(|t| t as i64),
                    event.duration_seconds as i64,
                    event.track_duration_seconds as i64,
                    if event.completed { 1 } else { 0 },
                    event.seek_count as i64,
                    event.pause_count as i64,
                    event.playback_context,
                    event.client_type,
                    event.date as i64,
                ],
            )?
        };

        let created = result > 0;
        let id = if created {
            conn.last_insert_rowid() as usize
        } else {
            // If not created (duplicate session_id), fetch the existing id
            conn.query_row(
                &format!(
                    "SELECT id FROM {} WHERE session_id = ?1",
                    LISTENING_EVENTS_TABLE_V_6.name
                ),
                params![event.session_id],
                |row| row.get::<_, i64>(0),
            )? as usize
        };

        record_db_query("record_listening_event", start.elapsed());
        Ok((id, created))
    }

    fn get_user_listening_events(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ListeningEvent>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let limit_val = limit.unwrap_or(50).min(500) as i64;
        let offset_val = offset.unwrap_or(0) as i64;

        let mut stmt = conn.prepare(&format!(
            "SELECT id, user_id, track_id, session_id, started_at, ended_at,
             duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
             playback_context, client_type, date
             FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY started_at DESC
             LIMIT ?4 OFFSET ?5",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let events = stmt
            .query_map(
                params![user_id, start_date, end_date, limit_val, offset_val],
                |row| {
                    Ok(ListeningEvent {
                        id: Some(row.get::<_, i64>(0)? as usize),
                        user_id: row.get::<_, i64>(1)? as usize,
                        track_id: row.get(2)?,
                        session_id: row.get(3)?,
                        started_at: row.get::<_, i64>(4)? as u64,
                        ended_at: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                        duration_seconds: row.get::<_, i64>(6)? as u32,
                        track_duration_seconds: row.get::<_, i64>(7)? as u32,
                        completed: row.get::<_, i64>(8)? != 0,
                        seek_count: row.get::<_, i64>(9)? as u32,
                        pause_count: row.get::<_, i64>(10)? as u32,
                        playback_context: row.get(11)?,
                        client_type: row.get(12)?,
                        date: row.get::<_, i64>(13)? as u32,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_listening_events", start.elapsed());
        Ok(events)
    }

    fn get_user_listening_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<ListeningSummary> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let summary = conn.query_row(
            &format!(
                "SELECT
                    COUNT(*) as total_plays,
                    COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                    COALESCE(SUM(completed), 0) as completed_plays,
                    COUNT(DISTINCT track_id) as unique_tracks
                 FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![user_id, start_date, end_date],
            |row| {
                Ok(ListeningSummary {
                    user_id: Some(user_id),
                    total_plays: row.get::<_, i64>(0)? as u64,
                    total_duration_seconds: row.get::<_, i64>(1)? as u64,
                    completed_plays: row.get::<_, i64>(2)? as u64,
                    unique_tracks: row.get::<_, i64>(3)? as u64,
                })
            },
        )?;

        record_db_query("get_user_listening_summary", start.elapsed());
        Ok(summary)
    }

    fn get_user_listening_history(
        &self,
        user_id: usize,
        limit: usize,
    ) -> Result<Vec<UserListeningHistoryEntry>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                track_id,
                MAX(started_at) as last_played_at,
                COUNT(*) as play_count,
                SUM(duration_seconds) as total_duration_seconds
             FROM {} WHERE user_id = ?1
             GROUP BY track_id
             ORDER BY last_played_at DESC
             LIMIT ?2",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let entries = stmt
            .query_map(params![user_id, limit as i64], |row| {
                Ok(UserListeningHistoryEntry {
                    track_id: row.get(0)?,
                    last_played_at: row.get::<_, i64>(1)? as u64,
                    play_count: row.get::<_, i64>(2)? as u64,
                    total_duration_seconds: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_listening_history", start.elapsed());
        Ok(entries)
    }

    fn get_track_listening_stats(
        &self,
        track_id: &str,
        start_date: u32,
        end_date: u32,
    ) -> Result<TrackListeningStats> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let stats = conn.query_row(
            &format!(
                "SELECT
                    COUNT(*) as play_count,
                    COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                    COALESCE(SUM(completed), 0) as completed_count,
                    COUNT(DISTINCT user_id) as unique_listeners
                 FROM {} WHERE track_id = ?1 AND date >= ?2 AND date <= ?3",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![track_id, start_date, end_date],
            |row| {
                Ok(TrackListeningStats {
                    track_id: track_id.to_string(),
                    play_count: row.get::<_, i64>(0)? as u64,
                    total_duration_seconds: row.get::<_, i64>(1)? as u64,
                    completed_count: row.get::<_, i64>(2)? as u64,
                    unique_listeners: row.get::<_, i64>(3)? as u64,
                })
            },
        )?;

        record_db_query("get_track_listening_stats", start.elapsed());
        Ok(stats)
    }

    fn get_daily_listening_stats(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<DailyListeningStats>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                date,
                COUNT(*) as total_plays,
                COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                COALESCE(SUM(completed), 0) as completed_plays,
                COUNT(DISTINCT user_id) as unique_users,
                COUNT(DISTINCT track_id) as unique_tracks
             FROM {} WHERE date >= ?1 AND date <= ?2
             GROUP BY date
             ORDER BY date DESC",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let stats = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(DailyListeningStats {
                    date: row.get::<_, i64>(0)? as u32,
                    total_plays: row.get::<_, i64>(1)? as u64,
                    total_duration_seconds: row.get::<_, i64>(2)? as u64,
                    completed_plays: row.get::<_, i64>(3)? as u64,
                    unique_users: row.get::<_, i64>(4)? as u64,
                    unique_tracks: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_daily_listening_stats", start.elapsed());
        Ok(stats)
    }

    fn get_top_tracks(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<TrackListeningStats>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                track_id,
                COUNT(*) as play_count,
                COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                COALESCE(SUM(completed), 0) as completed_count,
                COUNT(DISTINCT user_id) as unique_listeners
             FROM {} WHERE date >= ?1 AND date <= ?2
             GROUP BY track_id
             ORDER BY play_count DESC
             LIMIT ?3",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let stats = stmt
            .query_map(params![start_date, end_date, limit as i64], |row| {
                Ok(TrackListeningStats {
                    track_id: row.get(0)?,
                    play_count: row.get::<_, i64>(1)? as u64,
                    total_duration_seconds: row.get::<_, i64>(2)? as u64,
                    completed_count: row.get::<_, i64>(3)? as u64,
                    unique_listeners: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_top_tracks", start.elapsed());
        Ok(stats)
    }

    fn prune_listening_events(&self, older_than_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Calculate the cutoff date in YYYYMMDD format
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_secs = now - (older_than_days as u64 * 24 * 60 * 60);

        // Convert to YYYYMMDD format
        let cutoff_date = {
            let datetime = chrono::DateTime::from_timestamp(cutoff_secs as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());
            datetime.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0)
        };

        let deleted = conn.execute(
            &format!(
                "DELETE FROM {} WHERE date < ?1",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![cutoff_date],
        )?;

        record_db_query("prune_listening_events", start.elapsed());
        Ok(deleted)
    }
}

impl UserSettingsStore for SqliteUserStore {
    fn get_user_setting(&self, user_id: usize, key: &str) -> Result<Option<UserSetting>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT setting_value FROM user_settings WHERE user_id = ?1 AND setting_key = ?2",
            params![user_id, key],
            |row| row.get::<usize, Option<String>>(0),
        );

        record_db_query("get_user_setting", start.elapsed());

        match result {
            Ok(Some(value)) => {
                let setting = UserSetting::from_key_value(key, &value)
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(Some(setting))
            }
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_user_setting(&self, user_id: usize, setting: UserSetting) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let key = setting.key();
        let value = setting.value_to_string();

        conn.execute(
            "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
             VALUES (?1, ?2, ?3, (cast(strftime('%s','now') as int)))
             ON CONFLICT(user_id, setting_key) DO UPDATE SET
                 setting_value = excluded.setting_value,
                 updated = excluded.updated",
            params![user_id, key, value],
        )?;

        record_db_query("set_user_setting", start.elapsed());
        Ok(())
    }

    fn get_all_user_settings(&self, user_id: usize) -> Result<Vec<UserSetting>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt =
            conn.prepare("SELECT setting_key, setting_value FROM user_settings WHERE user_id = ?1")?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            let (key, value) = row?;
            // Skip unknown keys for forward compatibility
            if let Ok(setting) = UserSetting::from_key_value(&key, &value) {
                settings.push(setting);
            }
        }

        record_db_query("get_all_user_settings", start.elapsed());
        Ok(settings)
    }
}

impl user_store::DeviceStore for SqliteUserStore {
    fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT ... ON CONFLICT for upsert semantics
        conn.execute(
            "INSERT INTO device (device_uuid, device_type, device_name, os_info, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(device_uuid) DO UPDATE SET
                device_type = ?2,
                device_name = ?3,
                os_info = ?4,
                last_seen = ?5",
            params![
                registration.device_uuid,
                registration.device_type.as_str(),
                registration.device_name,
                registration.os_info,
                now,
            ],
        )?;

        // Get the device ID (either newly created or existing)
        let device_id: usize = conn.query_row(
            "SELECT id FROM device WHERE device_uuid = ?1",
            params![registration.device_uuid],
            |row| row.get(0),
        )?;

        record_db_query("register_or_update_device", start.elapsed());
        Ok(device_id)
    }

    fn get_device(&self, _device_id: usize) -> Result<Option<device::Device>> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn get_device_by_uuid(&self, _device_uuid: &str) -> Result<Option<device::Device>> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn get_user_devices(&self, _user_id: usize) -> Result<Vec<device::Device>> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn associate_device_with_user(&self, _device_id: usize, _user_id: usize) -> Result<()> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn touch_device(&self, _device_id: usize) -> Result<()> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn prune_orphaned_devices(&self, _inactive_for_days: u32) -> Result<usize> {
        todo!("DeviceStore implementation pending - Phase 5")
    }

    fn enforce_user_device_limit(&self, _user_id: usize, _max_devices: usize) -> Result<usize> {
        todo!("DeviceStore implementation pending - Phase 5")
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use chrono;
    use tempfile::TempDir;

    fn create_tmp_store() -> (SqliteUserStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test.db");
        let store = SqliteUserStore::new(&temp_file_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_create_user() {
        let (store, _temp_dir) = create_tmp_store();

        let user_id = store.create_user("test_user").unwrap();
        assert_eq!(user_id, 1);

        let duplicate_id = store.create_user("test_user");
        assert!(duplicate_id.is_err());
    }

    #[test]
    fn test_cannot_create_linked_content_without_user() {
        let (store, _temp_dir) = create_tmp_store();

        let result = store.set_user_liked_content(1, "test_content", LikedContentType::Album, true);
        assert!(result.is_err());
    }

    #[test]
    fn creates_liked_content() {
        let (store, _temp_dir) = create_tmp_store();

        let test_user_id = store.create_user("test_user").unwrap();
        store
            .set_user_liked_content(test_user_id, "test_content", LikedContentType::Artist, true)
            .unwrap();

        assert!(store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap()
            .unwrap());

        store
            .set_user_liked_content(test_user_id, "test_content", LikedContentType::Album, false)
            .unwrap();

        assert!(!store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap()
            .unwrap());
    }

    #[test]
    fn handles_playlists() {
        // First create a user
        let (store, _temp_dir) = create_tmp_store();
        let user_handle = "test_handle";
        let test_user_id = store.create_user(&user_handle).unwrap();

        // Create a playlist
        let plyalist_id = store
            .create_user_playlist(
                test_user_id,
                "test_playlist",
                test_user_id,
                vec!["track1".to_string(), "track2".to_string()],
            )
            .unwrap();

        let user_playslits_ids = store.get_user_playlists(test_user_id).unwrap();
        assert_eq!(user_playslits_ids, vec![plyalist_id.clone()]);

        let playlist2_id = store
            .create_user_playlist(
                test_user_id,
                "test_playlist2",
                test_user_id,
                vec!["track1".to_string(), "track2".to_string()],
            )
            .unwrap();

        let user_playslits_ids = store.get_user_playlists(test_user_id).unwrap();

        assert_eq!(
            user_playslits_ids,
            vec![plyalist_id.clone(), playlist2_id.clone()]
        );

        store
            .delete_user_playlist(&plyalist_id, test_user_id)
            .unwrap();
        store
            .delete_user_playlist(&playlist2_id, test_user_id)
            .unwrap();

        assert_eq!(store.get_user_playlists(test_user_id).unwrap().len(), 0,);
    }

    #[test]
    fn test_migration_v3_to_v4() {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test_migration.db");

        // Create a V3 database manually
        {
            let conn = Connection::open(&temp_file_path).unwrap();
            VERSIONED_SCHEMAS[3].create(&conn).unwrap(); // V3 is at index 3

            // Add some test data
            conn.execute(
                "INSERT INTO user (handle) VALUES (?1)",
                params!["test_user"],
            )
            .unwrap();
            let user_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO liked_content (user_id, content_id, content_type) VALUES (?1, ?2, ?3)",
                params![user_id, "test_content_id", 1],
            )
            .unwrap();

            conn.execute(
                "INSERT INTO user_playlist (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
                params!["playlist123", user_id, "Test Playlist", user_id],
            )
            .unwrap();

            // Verify we're at V3
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(db_version, BASE_DB_VERSION as i64 + 3);
        }

        // Now open with SqliteUserStore, which should trigger migration to latest (V8)
        let store = SqliteUserStore::new(&temp_file_path).unwrap();

        // Verify we're now at the latest version (V8)
        {
            let conn = store.conn.lock().unwrap();
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(db_version, BASE_DB_VERSION as i64 + 8);

            // Verify new tables exist
            let user_role_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_role'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(user_role_table_exists, 1);

            let user_extra_permission_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_extra_permission'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(user_extra_permission_table_exists, 1);

            // Verify indices exist with correct names
            let role_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_user_role_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(role_index_exists, 1);

            let permission_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_user_extra_permission_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(permission_index_exists, 1);

            // Verify listening_events table exists (V6)
            let listening_events_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='listening_events'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(listening_events_table_exists, 1);

            // Verify listening_events indices exist
            let listening_events_user_id_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_listening_events_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(listening_events_user_id_index_exists, 1);
        }

        // Verify old data is still intact
        let user_id = store.get_user_id("test_user").unwrap().unwrap();
        assert_eq!(user_id, 1);

        let liked_content = store
            .is_user_liked_content(user_id, "test_content_id")
            .unwrap()
            .unwrap();
        assert!(liked_content);

        let playlists = store.get_user_playlists(user_id).unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0], "playlist123");

        // Test new permission functionality
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);

        // Test adding extra permission
        let grant = PermissionGrant::Extra {
            start_time: SystemTime::now(),
            end_time: None,
            permission: Permission::EditCatalog,
            countdown: None,
        };
        let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
        assert!(permission_id > 0);

        // Test resolving permissions
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog)); // From Regular role
        assert!(permissions.contains(&Permission::EditCatalog)); // From extra permission
    }

    #[test]
    fn test_add_single_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add a single role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify the role was added
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_add_multiple_roles() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add multiple roles
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Verify both roles were added
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Regular));
        assert!(roles.contains(&UserRole::Admin));
    }

    #[test]
    fn test_add_duplicate_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add the same role twice
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify the role is only present once
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_remove_role_with_multiple_roles() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add multiple roles
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Remove one role
        store.remove_user_role(user_id, UserRole::Regular).unwrap();

        // Verify only Admin remains
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Admin);
    }

    #[test]
    fn test_remove_last_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add a single role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Remove the role
        store.remove_user_role(user_id, UserRole::Regular).unwrap();

        // Verify no roles remain
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 0);

        // Verify the database row was deleted
        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {} WHERE user_id = ?1", USER_ROLE_TABLE_V_4.name),
                params![user_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_remove_nonexistent_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add Regular role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Try to remove Admin role (not present)
        store.remove_user_role(user_id, UserRole::Admin).unwrap();

        // Verify Regular is still there
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_get_roles_with_comma_separated_string() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Manually insert comma-separated roles into the database
        let conn = store.conn.lock().unwrap();
        conn.execute(
            &format!("INSERT INTO {} (user_id, role) VALUES (?1, ?2)", USER_ROLE_TABLE_V_4.name),
            params![user_id, "Admin,Regular"],
        )
        .unwrap();
        drop(conn);

        // Verify both roles are parsed correctly
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Admin));
        assert!(roles.contains(&UserRole::Regular));
    }

    #[test]
    fn test_get_roles_with_spaces_in_comma_separated_string() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Manually insert comma-separated roles with spaces
        let conn = store.conn.lock().unwrap();
        conn.execute(
            &format!("INSERT INTO {} (user_id, role) VALUES (?1, ?2)", USER_ROLE_TABLE_V_4.name),
            params![user_id, "Admin, Regular"],
        )
        .unwrap();
        drop(conn);

        // Verify both roles are parsed correctly (spaces are trimmed)
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Admin));
        assert!(roles.contains(&UserRole::Regular));
    }

    #[test]
    fn test_role_permissions_resolution() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add Regular role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify Regular permissions
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog));
        assert!(permissions.contains(&Permission::LikeContent));
        assert!(permissions.contains(&Permission::OwnPlaylists));
        assert!(!permissions.contains(&Permission::EditCatalog));
        assert!(!permissions.contains(&Permission::ManagePermissions));

        // Add Admin role
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Verify Admin permissions are now present
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog));
        assert!(permissions.contains(&Permission::LikeContent));
        assert!(permissions.contains(&Permission::OwnPlaylists));
        assert!(permissions.contains(&Permission::EditCatalog));
        assert!(permissions.contains(&Permission::ManagePermissions));
        assert!(permissions.contains(&Permission::IssueContentDownload));
        assert!(permissions.contains(&Permission::RebootServer));
    }

    #[test]
    fn test_auth_token_last_used_update() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a token
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };

        store.add_user_auth_token(token.clone()).unwrap();

        // Verify last_used is initially None
        let retrieved_token = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert!(retrieved_token.last_used.is_none());

        // Update last_used timestamp
        store
            .update_user_auth_token_last_used_timestamp(&token.value)
            .unwrap();

        // Verify last_used is now set
        let updated_token = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert!(updated_token.last_used.is_some());
    }

    #[test]
    fn test_prune_unused_auth_tokens() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create an old token (simulate by manually inserting with old timestamp)
        let old_token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(old_token.clone()).unwrap();

        // Manually set the created timestamp to 10 days ago
        let conn = store.conn.lock().unwrap();
        let ten_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (10 * 24 * 60 * 60);
        conn.execute(
            "UPDATE auth_token SET created = ?1 WHERE value = ?2",
            params![ten_days_ago as i64, old_token.value.0],
        )
        .unwrap();
        drop(conn);

        // Create a recent token
        let recent_token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(recent_token.clone()).unwrap();

        // Verify both tokens exist
        assert!(store.get_user_auth_token(&old_token.value).unwrap().is_some());
        assert!(store.get_user_auth_token(&recent_token.value).unwrap().is_some());

        // Prune tokens older than 7 days
        let pruned = store.prune_unused_auth_tokens(7).unwrap();
        assert_eq!(pruned, 1);

        // Verify old token is gone and recent token remains
        assert!(store.get_user_auth_token(&old_token.value).unwrap().is_none());
        assert!(store.get_user_auth_token(&recent_token.value).unwrap().is_some());
    }

    #[test]
    fn test_prune_respects_last_used() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create an old token
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(token.clone()).unwrap();

        // Manually set the created timestamp to 10 days ago
        let conn = store.conn.lock().unwrap();
        let ten_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (10 * 24 * 60 * 60);
        conn.execute(
            "UPDATE auth_token SET created = ?1 WHERE value = ?2",
            params![ten_days_ago as i64, token.value.0],
        )
        .unwrap();
        drop(conn);

        // Update last_used to now (recent usage)
        store
            .update_user_auth_token_last_used_timestamp(&token.value)
            .unwrap();

        // Prune tokens older than 7 days
        let pruned = store.prune_unused_auth_tokens(7).unwrap();
        assert_eq!(pruned, 0);

        // Verify token still exists because it was recently used
        assert!(store.get_user_auth_token(&token.value).unwrap().is_some());
    }

    // Bandwidth tracking tests

    #[test]
    fn test_record_bandwidth_usage() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record initial bandwidth usage
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();

        // Verify the record was created
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].user_id, user_id);
        assert_eq!(records[0].date, 20241127);
        assert_eq!(records[0].endpoint_category, "stream");
        assert_eq!(records[0].bytes_sent, 1024);
        assert_eq!(records[0].request_count, 1);
    }

    #[test]
    fn test_record_bandwidth_aggregates_same_day_category() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth usage twice for same day/category
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 2048, 2)
            .unwrap();

        // Verify values were aggregated (not duplicated)
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].bytes_sent, 3072); // 1024 + 2048
        assert_eq!(records[0].request_count, 3); // 1 + 2
    }

    #[test]
    fn test_record_bandwidth_different_categories() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth for different categories on same day
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "catalog", 512, 5)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "image", 2048, 2)
            .unwrap();

        // Verify separate records for each category
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_get_user_bandwidth_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth for different categories
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 10000, 10)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "catalog", 5000, 100)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241128, "stream", 15000, 15)
            .unwrap();

        // Get summary
        let summary = store
            .get_user_bandwidth_summary(user_id, 20241127, 20241128)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_bytes_sent, 30000); // 10000 + 5000 + 15000
        assert_eq!(summary.total_requests, 125); // 10 + 100 + 15

        // Check category breakdown
        let stream_stats = summary.by_category.get("stream").unwrap();
        assert_eq!(stream_stats.bytes_sent, 25000); // 10000 + 15000
        assert_eq!(stream_stats.request_count, 25); // 10 + 15

        let catalog_stats = summary.by_category.get("catalog").unwrap();
        assert_eq!(catalog_stats.bytes_sent, 5000);
        assert_eq!(catalog_stats.request_count, 100);
    }

    #[test]
    fn test_get_all_bandwidth_usage() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Record bandwidth for different users
        store
            .record_bandwidth_usage(user1_id, 20241127, "stream", 1000, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "catalog", 2000, 2)
            .unwrap();

        // Get all bandwidth usage
        let records = store
            .get_all_bandwidth_usage(20241127, 20241127)
            .unwrap();

        assert_eq!(records.len(), 2);
        // Records should include both users
        let user_ids: Vec<usize> = records.iter().map(|r| r.user_id).collect();
        assert!(user_ids.contains(&user1_id));
        assert!(user_ids.contains(&user2_id));
    }

    #[test]
    fn test_get_total_bandwidth_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Record bandwidth for different users
        store
            .record_bandwidth_usage(user1_id, 20241127, "stream", 1000, 10)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "stream", 2000, 20)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "catalog", 500, 5)
            .unwrap();

        // Get total summary
        let summary = store
            .get_total_bandwidth_summary(20241127, 20241127)
            .unwrap();

        assert_eq!(summary.user_id, None); // Total summary has no specific user
        assert_eq!(summary.total_bytes_sent, 3500); // 1000 + 2000 + 500
        assert_eq!(summary.total_requests, 35); // 10 + 20 + 5
    }

    #[test]
    fn test_bandwidth_date_range_filter() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth on different days
        store
            .record_bandwidth_usage(user_id, 20241125, "stream", 1000, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241126, "stream", 2000, 2)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 3000, 3)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241128, "stream", 4000, 4)
            .unwrap();

        // Query for subset of dates
        let records = store
            .get_user_bandwidth_usage(user_id, 20241126, 20241127)
            .unwrap();

        assert_eq!(records.len(), 2);
        let dates: Vec<u32> = records.iter().map(|r| r.date).collect();
        assert!(dates.contains(&20241126));
        assert!(dates.contains(&20241127));
        assert!(!dates.contains(&20241125));
        assert!(!dates.contains(&20241128));
    }

    #[test]
    fn test_bandwidth_usage_deleted_on_user_delete() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth usage
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();

        // Verify record exists
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);

        // Delete user (bandwidth_usage has ON DELETE CASCADE foreign key)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify bandwidth records were deleted with user
        let all_records = store
            .get_all_bandwidth_usage(20241127, 20241127)
            .unwrap();
        assert_eq!(all_records.len(), 0);
    }

    // ==================== Listening Events Tests ====================

    fn create_test_listening_event(user_id: usize, track_id: &str, date: u32) -> ListeningEvent {
        ListeningEvent {
            id: None,
            user_id,
            track_id: track_id.to_string(),
            session_id: None,
            started_at: 1732982400, // Some fixed timestamp
            ended_at: Some(1732982587),
            duration_seconds: 187,
            track_duration_seconds: 210,
            completed: true,
            seek_count: 2,
            pause_count: 1,
            playback_context: Some("album".to_string()),
            client_type: Some("android".to_string()),
            date,
        }
    }

    #[test]
    fn test_record_listening_event_basic() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = create_test_listening_event(user_id, "tra_12345", 20241201);
        let (id, created) = store.record_listening_event(event).unwrap();

        assert!(id > 0);
        assert!(created);
    }

    #[test]
    fn test_record_listening_event_deduplication_with_session_id() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let mut event = create_test_listening_event(user_id, "tra_12345", 20241201);
        event.session_id = Some("unique-session-uuid".to_string());

        // First insert should succeed
        let (id1, created1) = store.record_listening_event(event.clone()).unwrap();
        assert!(id1 > 0);
        assert!(created1);

        // Second insert with same session_id should be ignored (deduplication)
        let (id2, created2) = store.record_listening_event(event).unwrap();
        assert_eq!(id2, id1); // Same ID returned
        assert!(!created2); // Not created (duplicate)
    }

    #[test]
    fn test_record_listening_event_without_session_id_always_inserts() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = create_test_listening_event(user_id, "tra_12345", 20241201);

        // First insert
        let (id1, created1) = store.record_listening_event(event.clone()).unwrap();
        assert!(created1);

        // Second insert without session_id should create new record
        let (id2, created2) = store.record_listening_event(event).unwrap();
        assert!(created2);
        assert_ne!(id1, id2); // Different IDs
    }

    #[test]
    fn test_get_user_listening_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record events on different dates
        let event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        let event2 = create_test_listening_event(user_id, "tra_002", 20241202);
        let event3 = create_test_listening_event(user_id, "tra_003", 20241203);

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        // Get all events
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, None, None)
            .unwrap();
        assert_eq!(events.len(), 3);

        // Get events for specific date range
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241202, None, None)
            .unwrap();
        assert_eq!(events.len(), 2);

        // Test pagination
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, Some(2), None)
            .unwrap();
        assert_eq!(events.len(), 2);

        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, Some(2), Some(2))
            .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_get_user_listening_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record completed event
        let mut event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        event1.duration_seconds = 200;
        event1.completed = true;

        // Record incomplete event
        let mut event2 = create_test_listening_event(user_id, "tra_002", 20241201);
        event2.duration_seconds = 50;
        event2.completed = false;

        // Record another play of the same track
        let mut event3 = create_test_listening_event(user_id, "tra_001", 20241201);
        event3.duration_seconds = 180;
        event3.completed = true;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241201)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_plays, 3);
        assert_eq!(summary.total_duration_seconds, 430); // 200 + 50 + 180
        assert_eq!(summary.completed_plays, 2);
        assert_eq!(summary.unique_tracks, 2); // tra_001 and tra_002
    }

    #[test]
    fn test_get_user_listening_history() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record events - tra_001 played twice, tra_002 played once
        let mut event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        event1.started_at = 1000;
        event1.duration_seconds = 100;

        let mut event2 = create_test_listening_event(user_id, "tra_002", 20241201);
        event2.started_at = 2000;
        event2.duration_seconds = 150;

        let mut event3 = create_test_listening_event(user_id, "tra_001", 20241201);
        event3.started_at = 3000;
        event3.duration_seconds = 120;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let history = store.get_user_listening_history(user_id, 10).unwrap();

        assert_eq!(history.len(), 2); // 2 unique tracks

        // Should be ordered by last_played_at descending
        assert_eq!(history[0].track_id, "tra_001");
        assert_eq!(history[0].play_count, 2);
        assert_eq!(history[0].total_duration_seconds, 220); // 100 + 120
        assert_eq!(history[0].last_played_at, 3000);

        assert_eq!(history[1].track_id, "tra_002");
        assert_eq!(history[1].play_count, 1);
        assert_eq!(history[1].total_duration_seconds, 150);
    }

    #[test]
    fn test_get_track_listening_stats() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // User 1 plays track twice
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.duration_seconds = 100;
        event1.completed = true;

        let mut event2 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event2.duration_seconds = 50;
        event2.completed = false;

        // User 2 plays track once
        let mut event3 = create_test_listening_event(user2_id, "tra_001", 20241201);
        event3.duration_seconds = 200;
        event3.completed = true;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let stats = store
            .get_track_listening_stats("tra_001", 20241201, 20241201)
            .unwrap();

        assert_eq!(stats.track_id, "tra_001");
        assert_eq!(stats.play_count, 3);
        assert_eq!(stats.total_duration_seconds, 350); // 100 + 50 + 200
        assert_eq!(stats.completed_count, 2);
        assert_eq!(stats.unique_listeners, 2);
    }

    #[test]
    fn test_get_daily_listening_stats() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Day 1: user1 plays tra_001
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.duration_seconds = 100;
        event1.completed = true;

        // Day 1: user2 plays tra_002
        let mut event2 = create_test_listening_event(user2_id, "tra_002", 20241201);
        event2.duration_seconds = 150;
        event2.completed = false;

        // Day 2: user1 plays tra_001 again
        let mut event3 = create_test_listening_event(user1_id, "tra_001", 20241202);
        event3.duration_seconds = 200;
        event3.completed = true;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let daily_stats = store.get_daily_listening_stats(20241201, 20241202).unwrap();

        assert_eq!(daily_stats.len(), 2);

        // Day 1 stats
        let day1 = daily_stats.iter().find(|d| d.date == 20241201).unwrap();
        assert_eq!(day1.total_plays, 2);
        assert_eq!(day1.total_duration_seconds, 250); // 100 + 150
        assert_eq!(day1.completed_plays, 1);
        assert_eq!(day1.unique_users, 2);
        assert_eq!(day1.unique_tracks, 2);

        // Day 2 stats
        let day2 = daily_stats.iter().find(|d| d.date == 20241202).unwrap();
        assert_eq!(day2.total_plays, 1);
        assert_eq!(day2.total_duration_seconds, 200);
        assert_eq!(day2.completed_plays, 1);
        assert_eq!(day2.unique_users, 1);
        assert_eq!(day2.unique_tracks, 1);
    }

    #[test]
    fn test_get_top_tracks() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // tra_001: 3 plays
        for _ in 0..3 {
            let event = create_test_listening_event(user_id, "tra_001", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_002: 5 plays
        for _ in 0..5 {
            let event = create_test_listening_event(user_id, "tra_002", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_003: 1 play
        let event = create_test_listening_event(user_id, "tra_003", 20241201);
        store.record_listening_event(event).unwrap();

        let top_tracks = store.get_top_tracks(20241201, 20241201, 10).unwrap();

        assert_eq!(top_tracks.len(), 3);
        // Should be ordered by play_count descending
        assert_eq!(top_tracks[0].track_id, "tra_002");
        assert_eq!(top_tracks[0].play_count, 5);
        assert_eq!(top_tracks[1].track_id, "tra_001");
        assert_eq!(top_tracks[1].play_count, 3);
        assert_eq!(top_tracks[2].track_id, "tra_003");
        assert_eq!(top_tracks[2].play_count, 1);

        // Test limit
        let top_tracks = store.get_top_tracks(20241201, 20241201, 2).unwrap();
        assert_eq!(top_tracks.len(), 2);
    }

    #[test]
    fn test_prune_listening_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get today's date in YYYYMMDD format
        let today = chrono::Utc::now();
        let today_date: u32 = today.format("%Y%m%d").to_string().parse().unwrap();

        // Calculate old date (60 days ago)
        let old_date = (today - chrono::Duration::days(60))
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap();

        // Calculate recent date (5 days ago)
        let recent_date = (today - chrono::Duration::days(5))
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap();

        // Record old event (60 days ago)
        let old_event = create_test_listening_event(user_id, "tra_old", old_date);
        store.record_listening_event(old_event).unwrap();

        // Record recent event (5 days ago)
        let recent_event = create_test_listening_event(user_id, "tra_recent", recent_date);
        store.record_listening_event(recent_event).unwrap();

        // Verify both exist
        let all_events = store
            .get_user_listening_events(user_id, old_date, today_date, None, None)
            .unwrap();
        assert_eq!(all_events.len(), 2);

        // Prune events older than 30 days (should delete the 60-day-old event)
        let pruned = store.prune_listening_events(30).unwrap();
        assert_eq!(pruned, 1);

        // Verify only recent event remains
        let remaining_events = store
            .get_user_listening_events(user_id, old_date, today_date, None, None)
            .unwrap();
        assert_eq!(remaining_events.len(), 1);
        assert_eq!(remaining_events[0].track_id, "tra_recent");
    }

    #[test]
    fn test_listening_events_deleted_on_user_delete() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record listening event
        let event = create_test_listening_event(user_id, "tra_001", 20241201);
        store.record_listening_event(event).unwrap();

        // Verify event exists
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);

        // Delete user (listening_events has ON DELETE CASCADE)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify events were deleted with user
        // Need to check directly in DB since user no longer exists
        {
            let conn = store.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM listening_events WHERE user_id = ?1",
                    params![user_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn test_listening_event_with_minimal_fields() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create event with only required fields, optional fields as None
        let event = ListeningEvent {
            id: None,
            user_id,
            track_id: "tra_minimal".to_string(),
            session_id: None,
            started_at: 1732982400,
            ended_at: None,
            duration_seconds: 100,
            track_duration_seconds: 200,
            completed: false,
            seek_count: 0,
            pause_count: 0,
            playback_context: None,
            client_type: None,
            date: 20241201,
        };

        let (id, created) = store.record_listening_event(event).unwrap();
        assert!(id > 0);
        assert!(created);

        // Verify we can retrieve it
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].client_type.is_none());
        assert!(events[0].playback_context.is_none());
        assert!(events[0].ended_at.is_none());
    }

    #[test]
    fn test_listening_event_foreign_key_constraint() {
        let (store, _temp_dir) = create_tmp_store();

        // Try to insert event for non-existent user
        let event = create_test_listening_event(99999, "tra_001", 20241201);
        let result = store.record_listening_event(event);

        // Should fail due to foreign key constraint
        assert!(result.is_err());
    }

    #[test]
    fn test_get_user_listening_events_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Query with no events
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241231, None, None)
            .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_user_listening_events_user_isolation() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // User 1 listens to track A
        let event1 = create_test_listening_event(user1_id, "tra_user1", 20241201);
        store.record_listening_event(event1).unwrap();

        // User 2 listens to track B
        let event2 = create_test_listening_event(user2_id, "tra_user2", 20241201);
        store.record_listening_event(event2).unwrap();

        // User 1 should only see their events
        let user1_events = store
            .get_user_listening_events(user1_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(user1_events.len(), 1);
        assert_eq!(user1_events[0].track_id, "tra_user1");

        // User 2 should only see their events
        let user2_events = store
            .get_user_listening_events(user2_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(user2_events.len(), 1);
        assert_eq!(user2_events[0].track_id, "tra_user2");
    }

    #[test]
    fn test_get_user_listening_summary_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get summary with no events
        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241231)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_plays, 0);
        assert_eq!(summary.total_duration_seconds, 0);
        assert_eq!(summary.completed_plays, 0);
        assert_eq!(summary.unique_tracks, 0);
    }

    #[test]
    fn test_get_user_listening_history_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let history = store.get_user_listening_history(user_id, 10).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_get_user_listening_history_respects_limit() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 5 different tracks
        for i in 0..5 {
            let event = create_test_listening_event(user_id, &format!("tra_{:03}", i), 20241201);
            store.record_listening_event(event).unwrap();
        }

        // Request only 3
        let history = store.get_user_listening_history(user_id, 3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_get_track_listening_stats_nonexistent_track() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record some events for a different track
        let event = create_test_listening_event(user_id, "tra_exists", 20241201);
        store.record_listening_event(event).unwrap();

        // Query stats for non-existent track
        let stats = store
            .get_track_listening_stats("tra_nonexistent", 20241201, 20241201)
            .unwrap();

        assert_eq!(stats.track_id, "tra_nonexistent");
        assert_eq!(stats.play_count, 0);
        assert_eq!(stats.total_duration_seconds, 0);
        assert_eq!(stats.completed_count, 0);
        assert_eq!(stats.unique_listeners, 0);
    }

    #[test]
    fn test_get_daily_listening_stats_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let _user_id = store.create_user("test_user").unwrap();

        // Query stats for date range with no events
        let daily_stats = store.get_daily_listening_stats(20241201, 20241231).unwrap();
        assert!(daily_stats.is_empty());
    }

    #[test]
    fn test_get_top_tracks_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let _user_id = store.create_user("test_user").unwrap();

        let top_tracks = store.get_top_tracks(20241201, 20241231, 10).unwrap();
        assert!(top_tracks.is_empty());
    }

    #[test]
    fn test_prune_listening_events_nothing_to_prune() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get today's date
        let today = chrono::Utc::now();
        let today_date: u32 = today.format("%Y%m%d").to_string().parse().unwrap();

        // Record only recent events
        let recent_event = create_test_listening_event(user_id, "tra_recent", today_date);
        store.record_listening_event(recent_event).unwrap();

        // Prune events older than 30 days - nothing should be pruned
        let pruned = store.prune_listening_events(30).unwrap();
        assert_eq!(pruned, 0);

        // Verify event still exists
        let events = store
            .get_user_listening_events(user_id, today_date, today_date, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_session_id_uniqueness_across_users() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Same session_id for different users should both succeed
        // (session_id is globally unique, not per-user)
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.session_id = Some("shared-session-id".to_string());

        let mut event2 = create_test_listening_event(user2_id, "tra_001", 20241201);
        event2.session_id = Some("shared-session-id".to_string());

        let (_, created1) = store.record_listening_event(event1).unwrap();
        assert!(created1);

        // Second insert with same session_id should be deduplicated
        // even for different user (session_id is globally unique)
        let (_, created2) = store.record_listening_event(event2).unwrap();
        assert!(!created2);
    }

    #[test]
    fn test_get_user_listening_events_ordering() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Insert events with different started_at times
        let mut event1 = create_test_listening_event(user_id, "tra_first", 20241201);
        event1.started_at = 1000;

        let mut event2 = create_test_listening_event(user_id, "tra_third", 20241201);
        event2.started_at = 3000;

        let mut event3 = create_test_listening_event(user_id, "tra_second", 20241201);
        event3.started_at = 2000;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();

        // Should be ordered by started_at descending (most recent first)
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].track_id, "tra_third");
        assert_eq!(events[1].track_id, "tra_second");
        assert_eq!(events[2].track_id, "tra_first");
    }

    #[test]
    fn test_completion_calculation_boundary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Exactly 90% should be complete
        let mut event_90 = create_test_listening_event(user_id, "tra_90", 20241201);
        event_90.duration_seconds = 90;
        event_90.track_duration_seconds = 100;
        event_90.completed = true; // 90/100 = 0.90 = exactly 90%

        // 89% should not be complete
        let mut event_89 = create_test_listening_event(user_id, "tra_89", 20241201);
        event_89.duration_seconds = 89;
        event_89.track_duration_seconds = 100;
        event_89.completed = false; // 89/100 = 0.89 < 90%

        store.record_listening_event(event_90).unwrap();
        store.record_listening_event(event_89).unwrap();

        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241201)
            .unwrap();

        assert_eq!(summary.total_plays, 2);
        assert_eq!(summary.completed_plays, 1); // Only the 90% one
    }

    #[test]
    fn test_get_top_tracks_with_ties() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 3 tracks with same play count
        for track in &["tra_a", "tra_b", "tra_c"] {
            for _ in 0..5 {
                let event = create_test_listening_event(user_id, track, 20241201);
                store.record_listening_event(event).unwrap();
            }
        }

        let top_tracks = store.get_top_tracks(20241201, 20241201, 10).unwrap();

        assert_eq!(top_tracks.len(), 3);
        // All should have 5 plays
        for track in &top_tracks {
            assert_eq!(track.play_count, 5);
        }
    }

    #[test]
    fn test_daily_stats_multiple_days_gap() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Events on day 1 and day 5, nothing in between
        let event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        let event2 = create_test_listening_event(user_id, "tra_002", 20241205);

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();

        let daily_stats = store.get_daily_listening_stats(20241201, 20241205).unwrap();

        // Should only have 2 entries (days with actual events)
        assert_eq!(daily_stats.len(), 2);

        let dates: Vec<u32> = daily_stats.iter().map(|d| d.date).collect();
        assert!(dates.contains(&20241201));
        assert!(dates.contains(&20241205));
        // Days 2, 3, 4 should not be in results
        assert!(!dates.contains(&20241202));
        assert!(!dates.contains(&20241203));
        assert!(!dates.contains(&20241204));
    }

    // ==================== User Settings Tests ====================

    #[test]
    fn test_get_setting_returns_none_when_not_set() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_setting_returns_none_for_unknown_key() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let result = store.get_user_setting(user_id, "unknown_key").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_set_and_get_setting() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();

        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert_eq!(result, Some(UserSetting::DirectDownloadsEnabled(true)));
    }

    #[test]
    fn test_set_setting_overwrites_existing() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(false))
            .unwrap();
        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();

        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert_eq!(result, Some(UserSetting::DirectDownloadsEnabled(true)));
    }

    #[test]
    fn test_get_all_user_settings_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let settings = store.get_all_user_settings(user_id).unwrap();
        assert!(settings.is_empty());
    }

    #[test]
    fn test_get_all_user_settings_returns_known_settings() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();

        let settings = store.get_all_user_settings(user_id).unwrap();
        assert_eq!(settings.len(), 1);
        assert!(settings.contains(&UserSetting::DirectDownloadsEnabled(true)));
    }

    #[test]
    fn test_get_all_user_settings_skips_unknown_keys() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Set a known setting
        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();

        // Manually insert an unknown setting directly into the database
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
                 VALUES (?1, ?2, ?3, 0)",
                params![user_id, "unknown_future_setting", "some_value"],
            )
            .unwrap();
        }

        // get_all_user_settings should skip the unknown key
        let settings = store.get_all_user_settings(user_id).unwrap();
        assert_eq!(settings.len(), 1);
        assert!(settings.contains(&UserSetting::DirectDownloadsEnabled(true)));
    }

    #[test]
    fn test_settings_are_user_specific() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        store
            .set_user_setting(user1_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();
        store
            .set_user_setting(user2_id, UserSetting::DirectDownloadsEnabled(false))
            .unwrap();

        let user1_value = store
            .get_user_setting(user1_id, "enable_direct_downloads")
            .unwrap();
        let user2_value = store
            .get_user_setting(user2_id, "enable_direct_downloads")
            .unwrap();

        assert_eq!(user1_value, Some(UserSetting::DirectDownloadsEnabled(true)));
        assert_eq!(
            user2_value,
            Some(UserSetting::DirectDownloadsEnabled(false))
        );
    }

    #[test]
    fn test_settings_deleted_with_user() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();

        // Delete the user via direct SQL (CASCADE should delete settings)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify settings are gone by checking the table directly
        {
            let conn = store.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM user_settings WHERE user_id = ?1",
                    params![user_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn test_enable_direct_downloads_setting_lifecycle() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Default should be None (not set)
        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert!(result.is_none());

        // Set to true
        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(true))
            .unwrap();
        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert_eq!(result, Some(UserSetting::DirectDownloadsEnabled(true)));

        // Set to false
        store
            .set_user_setting(user_id, UserSetting::DirectDownloadsEnabled(false))
            .unwrap();
        let result = store
            .get_user_setting(user_id, "enable_direct_downloads")
            .unwrap();
        assert_eq!(result, Some(UserSetting::DirectDownloadsEnabled(false)));
    }
}
