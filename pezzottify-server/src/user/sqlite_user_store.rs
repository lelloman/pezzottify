#![allow(dead_code)]

use crate::server::metrics::record_db_query;
use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema, BASE_DB_VERSION,
    DEFAULT_TIMESTAMP,
};
use crate::user::device::{
    Device, DeviceRegistration, DeviceShareMode, DeviceSharePolicy, DeviceType,
};
use crate::user::permissions::UserRole;
use crate::user::user_models::{
    BandwidthSummary, BandwidthUsage, CategoryBandwidth, DailyListeningStats, ListeningEvent,
    ListeningSummary, TrackListeningStats, TrackPlayCount, UserListeningHistoryEntry,
};
use crate::user::user_store::{UserBandwidthStore, UserListeningStore, UserSettingsStore};
use crate::user::*;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::{HashMap, HashSet};
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
use sha2::{Digest, Sha256};

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

/// V 12 - Adds oidc_subject column for OIDC authentication
const USER_TABLE_V_12: Table = Table {
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
        sqlite_column!("oidc_subject", &SqlType::Text, is_unique = true),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_user_handle", "handle"),
        ("idx_user_oidc_subject", "oidc_subject"),
    ],
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
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!(
            "device_uuid",
            &SqlType::Text,
            non_null = true,
            is_unique = true
        ),
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

/// V 13
/// Device share policy table
const DEVICE_SHARE_POLICY_TABLE_V_13: Table = Table {
    name: "device_share_policy",
    columns: &[
        sqlite_column!(
            "device_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "device",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("mode", &SqlType::Text, non_null = true),
        sqlite_column!(
            "updated_at",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[&["device_id"]],
    indices: &[("idx_device_share_policy_device", "device_id")],
};

/// V 13
/// Device share rules table
const DEVICE_SHARE_RULE_TABLE_V_13: Table = Table {
    name: "device_share_rule",
    columns: &[
        sqlite_column!(
            "device_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "device",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("rule_type", &SqlType::Text, non_null = true),
        sqlite_column!("subject_type", &SqlType::Text, non_null = true),
        sqlite_column!("subject_value", &SqlType::Text, non_null = true),
        sqlite_column!(
            "created_at",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[&["device_id", "rule_type", "subject_type", "subject_value"]],
    indices: &[("idx_device_share_rule_device", "device_id")],
};

/// V 9
/// User events table - append-only log for multi-device sync
const USER_EVENTS_TABLE_V_9: Table = Table {
    name: "user_events",
    columns: &[
        sqlite_column!(
            "seq",
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
        sqlite_column!("event_type", &SqlType::Text, non_null = true),
        sqlite_column!("payload", &SqlType::Text, non_null = true),
        sqlite_column!(
            "server_timestamp",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[("idx_user_events_user_seq", "user_id, seq")],
};

/// V 14
/// Adds stable operation identifiers for idempotent mutation batches.
const USER_EVENTS_TABLE_V_14: Table = Table {
    name: "user_events",
    columns: &[
        sqlite_column!(
            "seq",
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
        sqlite_column!("event_type", &SqlType::Text, non_null = true),
        sqlite_column!("payload", &SqlType::Text, non_null = true),
        sqlite_column!(
            "server_timestamp",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!("operation_id", &SqlType::Text),
        sqlite_column!(
            "operation_index",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
    ],
    unique_constraints: &[&["user_id", "operation_id", "operation_index"]],
    indices: &[
        ("idx_user_events_user_seq", "user_id, seq"),
        (
            "idx_user_events_operation",
            "user_id, operation_id, operation_index",
        ),
    ],
};

/// V 11
/// User notifications table - stores notifications for each user
const USER_NOTIFICATIONS_TABLE_V_11: Table = Table {
    name: "user_notifications",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Text,
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
        sqlite_column!("notification_type", &SqlType::Text, non_null = true),
        sqlite_column!("title", &SqlType::Text, non_null = true),
        sqlite_column!("body", &SqlType::Text),
        sqlite_column!("data", &SqlType::Text, non_null = true), // JSON
        sqlite_column!("read_at", &SqlType::Integer),            // NULL = unread
        sqlite_column!(
            "created_at",
            &SqlType::Integer,
            non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
    ],
    unique_constraints: &[],
    indices: &[("idx_notifications_user_created", "user_id, created_at DESC")],
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

/// V15 stores only one-way token digests. Upgrading intentionally drops all
/// pre-V15 sessions so plaintext credentials are rotated rather than preserved.
const AUTH_TOKEN_TABLE_V_15: Table = Table {
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
        sqlite_column!(
            "token_hash",
            &SqlType::Text,
            non_null = true,
            is_unique = true
        ),
        sqlite_column!("token_id", &SqlType::Text, non_null = true),
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
        ("idx_auth_token_hash", "token_hash"),
        ("idx_auth_token_user", "user_id"),
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
            USER_PLAYLIST_TABLE_V_3.create(conn)?;
            USER_PLAYLIST_TRACKS_TABLE_V_3.create(conn)?;
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
            USER_ROLE_TABLE_V_4.create(conn)?;
            USER_EXTRA_PERMISSION_TABLE_V_4.create(conn)?;
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
            BANDWIDTH_USAGE_TABLE_V_5.create(conn)?;
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
            LISTENING_EVENTS_TABLE_V_6.create(conn)?;
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
            USER_SETTINGS_TABLE_V_7.create(conn)?;
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
            DEVICE_TABLE_V_8.create(conn)?;

            // Step 2: Delete all existing tokens (no real users yet, per plan)
            conn.execute("DELETE FROM auth_token", [])?;

            // Step 3: Recreate auth_token with device_id column
            // SQLite doesn't support ADD COLUMN with NOT NULL and FK well
            conn.execute("DROP TABLE auth_token", [])?;
            AUTH_TOKEN_TABLE_V_8.create(conn)?;

            Ok(())
        }),
    },
    VersionedSchema {
        version: 9,
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
            USER_EVENTS_TABLE_V_9,
        ],
        migration: Some(|conn: &Connection| {
            USER_EVENTS_TABLE_V_9.create(conn)?;
            Ok(())
        }),
    },
    // V10: No-op migration to maintain compatibility with databases
    // that were migrated when the direct download feature was removed
    VersionedSchema {
        version: 10,
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
            USER_EVENTS_TABLE_V_9,
        ],
        migration: Some(|_conn: &Connection| Ok(())),
    },
    // V11: Add user notifications table
    VersionedSchema {
        version: 11,
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
            USER_EVENTS_TABLE_V_9,
            USER_NOTIFICATIONS_TABLE_V_11,
        ],
        migration: Some(|conn: &Connection| {
            USER_NOTIFICATIONS_TABLE_V_11.create(conn)?;
            Ok(())
        }),
    },
    // V12: Add oidc_subject column to user table for OIDC authentication
    VersionedSchema {
        version: 12,
        tables: &[
            USER_TABLE_V_12,
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
            USER_EVENTS_TABLE_V_9,
            USER_NOTIFICATIONS_TABLE_V_11,
        ],
        migration: Some(|conn: &Connection| {
            // SQLite doesn't support adding UNIQUE constraint in ALTER TABLE,
            // so we add the column first, then create a unique index
            conn.execute("ALTER TABLE user ADD COLUMN oidc_subject TEXT", [])?;
            conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_oidc_subject ON user(oidc_subject)",
                [],
            )?;
            Ok(())
        }),
    },
    // V13: Add device share policy tables
    VersionedSchema {
        version: 13,
        tables: &[
            USER_TABLE_V_12,
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
            USER_EVENTS_TABLE_V_9,
            USER_NOTIFICATIONS_TABLE_V_11,
            DEVICE_SHARE_POLICY_TABLE_V_13,
            DEVICE_SHARE_RULE_TABLE_V_13,
        ],
        migration: Some(|conn: &Connection| {
            DEVICE_SHARE_POLICY_TABLE_V_13.create(conn)?;
            DEVICE_SHARE_RULE_TABLE_V_13.create(conn)?;
            Ok(())
        }),
    },
    // V14: Add idempotency metadata to user sync events.
    VersionedSchema {
        version: 14,
        tables: &[
            USER_TABLE_V_12,
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
            USER_EVENTS_TABLE_V_14,
            USER_NOTIFICATIONS_TABLE_V_11,
            DEVICE_SHARE_POLICY_TABLE_V_13,
            DEVICE_SHARE_RULE_TABLE_V_13,
        ],
        migration: Some(|conn: &Connection| {
            conn.execute("ALTER TABLE user_events ADD COLUMN operation_id TEXT", [])?;
            conn.execute(
                "ALTER TABLE user_events ADD COLUMN operation_index INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX idx_user_events_operation
                 ON user_events(user_id, operation_id, operation_index)",
                [],
            )?;
            Ok(())
        }),
    },
    // V15: Replace plaintext bearer credentials with SHA-256 digests. Existing
    // sessions are deliberately revoked during migration.
    VersionedSchema {
        version: 15,
        tables: &[
            USER_TABLE_V_12,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_15,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
            USER_PLAYLIST_TRACKS_TABLE_V_3,
            USER_ROLE_TABLE_V_4,
            USER_EXTRA_PERMISSION_TABLE_V_4,
            BANDWIDTH_USAGE_TABLE_V_5,
            LISTENING_EVENTS_TABLE_V_6,
            USER_SETTINGS_TABLE_V_7,
            DEVICE_TABLE_V_8,
            USER_EVENTS_TABLE_V_14,
            USER_NOTIFICATIONS_TABLE_V_11,
            DEVICE_SHARE_POLICY_TABLE_V_13,
            DEVICE_SHARE_RULE_TABLE_V_13,
        ],
        migration: Some(|conn: &Connection| {
            conn.execute("DROP TABLE auth_token", [])?;
            AUTH_TOKEN_TABLE_V_15.create(conn)?;
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
    pub fn new<T: AsRef<Path>>(
        db_path: T,
        db_registry: &crate::backup::DbRegistry,
    ) -> Result<Self> {
        let db_path_ref = db_path.as_ref().to_path_buf();
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

        db_registry.register(db_path_ref, &conn)?;

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

    // ========================================================================
    // Sync Event Log Methods
    // ========================================================================

    fn append_event_tx(
        tx: &Transaction<'_>,
        user_id: usize,
        event: &crate::user::sync_events::UserEvent,
        operation_id: Option<&str>,
        operation_index: i32,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let payload = serde_json::to_string(event)?;
        tx.execute(
            "INSERT INTO user_events
             (user_id, event_type, payload, operation_id, operation_index)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                user_id,
                event.event_type(),
                payload,
                operation_id,
                operation_index
            ],
        )?;

        let seq = tx.last_insert_rowid();
        let server_timestamp = tx.query_row(
            "SELECT server_timestamp FROM user_events WHERE seq = ?1",
            params![seq],
            |row| row.get(0),
        )?;
        Ok(crate::user::sync_events::StoredEvent {
            seq,
            operation_id: operation_id.map(str::to_owned),
            operation_index,
            event: event.clone(),
            server_timestamp,
        })
    }

    fn get_operation_events_tx(
        tx: &Transaction<'_>,
        user_id: usize,
        operation_id: Option<&str>,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let Some(operation_id) = operation_id else {
            return Ok(Vec::new());
        };
        let mut stmt = tx.prepare(
            "SELECT seq, payload, server_timestamp, operation_id, operation_index
             FROM user_events
             WHERE user_id = ?1 AND operation_id = ?2
             ORDER BY operation_index ASC",
        )?;
        let events = stmt
            .query_map(params![user_id, operation_id], |row| {
                let payload: String = row.get(1)?;
                let event = serde_json::from_str(&payload).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(crate::user::sync_events::StoredEvent {
                    seq: row.get(0)?,
                    event,
                    server_timestamp: row.get(2)?,
                    operation_id: row.get(3)?,
                    operation_index: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    /// Append an event to the user's event log.
    /// Returns the stored event with sequence number and server timestamp.
    pub fn append_event(
        &self,
        user_id: usize,
        event: &crate::user::sync_events::UserEvent,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let start = Instant::now();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let stored = Self::append_event_tx(&tx, user_id, event, None, 0)?;
        tx.commit()?;

        record_db_query("append_event", start.elapsed());
        Ok(stored)
    }

    /// Get events since a given sequence number.
    /// Returns events with seq > since_seq, ordered by seq ascending.
    pub fn get_events_since(
        &self,
        user_id: usize,
        since_seq: i64,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT seq, payload, server_timestamp, operation_id, operation_index
             FROM user_events
             WHERE user_id = ?1 AND seq > ?2
             ORDER BY seq ASC",
        )?;

        let events = stmt
            .query_map(params![user_id, since_seq], |row| {
                let seq: i64 = row.get(0)?;
                let payload: String = row.get(1)?;
                let server_timestamp: i64 = row.get(2)?;
                let event: crate::user::sync_events::UserEvent = serde_json::from_str(&payload)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                Ok(crate::user::sync_events::StoredEvent {
                    seq,
                    operation_id: row.get(3)?,
                    operation_index: row.get(4)?,
                    event,
                    server_timestamp,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Filter out events with deprecated data (e.g., removed settings)
        let events: Vec<_> = events
            .into_iter()
            .filter(|e| !e.event.is_deprecated())
            .collect();

        record_db_query("get_events_since", start.elapsed());
        Ok(events)
    }

    /// Get the current (latest) sequence number for a user.
    /// Returns 0 if no events exist.
    pub fn get_current_seq(&self, user_id: usize) -> Result<i64> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let seq: Option<i64> = conn
            .query_row(
                "SELECT MAX(seq) FROM user_events WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        record_db_query("get_current_seq", start.elapsed());
        Ok(seq.unwrap_or(0))
    }

    /// Get the minimum available sequence number for a user.
    /// Returns None if no events exist.
    /// Used to detect if requested sequence has been pruned.
    pub fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let seq: Option<i64> = conn
            .query_row(
                "SELECT MIN(seq) FROM user_events WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        record_db_query("get_min_seq", start.elapsed());
        Ok(seq)
    }

    /// Delete events older than the given Unix timestamp.
    /// Used for maintenance/pruning.
    /// Returns the number of deleted events.
    pub fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM user_events WHERE server_timestamp < ?1",
            params![before_timestamp],
        )?;

        record_db_query("prune_events_older_than", start.elapsed());
        Ok(deleted as u64)
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

    fn delete_user(&self, user_id: usize) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            &format!("DELETE FROM {} WHERE id = ?1", USER_TABLE_V_0.name),
            params![user_id],
        )?;
        Ok(rows_affected > 0)
    }

    fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, name FROM {} WHERE user_id = ?1",
            USER_PLAYLIST_TABLE_V_3.name
        ))?;
        let playlists = stmt
            .query_map(params![user_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(playlists)
    }

    fn get_user_handle(&self, user_id: usize) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
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
        let mut stmt = conn.prepare(&format!("SELECT handle FROM {}", USER_TABLE_V_0.name))?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(rows)
    }

    fn get_user_id(&self, user_handle: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
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

    fn get_user_id_by_oidc_subject(&self, oidc_subject: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id FROM {} WHERE oidc_subject = ?1",
            USER_TABLE_V_12.name
        ))?;
        match stmt.query_row(params![oidc_subject], |row| row.get(0)) {
            Ok(id) => {
                let id: i32 = id;
                Ok(Some(id as usize))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_user_oidc_subject(&self, user_id: usize, oidc_subject: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "UPDATE {} SET oidc_subject = ?1 WHERE id = ?2",
                USER_TABLE_V_12.name
            ),
            params![oidc_subject, user_id],
        )?;
        Ok(())
    }

    fn get_user_oidc_subject(&self, user_id: usize) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT oidc_subject FROM {} WHERE id = ?1",
            USER_TABLE_V_12.name
        ))?;
        match stmt.query_row(params![user_id], |row| row.get::<_, Option<String>>(0)) {
            Ok(subject) => Ok(subject),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn clear_user_oidc_subject(&self, user_id: usize) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "UPDATE {} SET oidc_subject = NULL WHERE id = ?1",
                USER_TABLE_V_12.name
            ),
            params![user_id],
        )?;
        Ok(())
    }

    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Result<Option<bool>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT COUNT(*) FROM {} WHERE user_id = ?1 AND content_id = ?2",
            LIKED_CONTENT_TABLE_V_2.name
        ))?;
        let count: i32 = stmt.query_row(params![user_id, content_id], |row| row.get(0))?;

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
                params![user_id, content_id, content_type.as_int()],
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
            .query_map(params![user_id, content_type.as_int()], |row| row.get(0))
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

        let playlist_user_id = tx
            .query_row(
                &format!(
                    "SELECT user_id FROM {} WHERE id = ?1",
                    USER_PLAYLIST_TABLE_V_3.name
                ),
                params![playlist_id],
                |row| row.get::<usize, usize>(0),
            )
            .optional()?
            .ok_or_else(super::UserServiceError::playlist_not_found)?;
        debug!("update_user_playlist({playlist_id}) found user_id: {playlist_user_id}",);
        if user_id != playlist_user_id {
            return Err(super::UserServiceError::playlist_not_found().into());
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
        let changed = conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1 AND user_id = ?2",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id, user_id],
        )?;
        if changed == 0 {
            return Err(super::UserServiceError::playlist_not_found().into());
        }
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
                user_id,
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
                debug!(
                    "get_user_roles: found role string '{}' for user_id={}",
                    role_str, user_id
                );
                Ok(role_str)
            })?
            .filter_map(|r| r.ok())
            .flat_map(|s| {
                s.split(',')
                    .map(|part| part.trim())
                    .filter_map(UserRole::from_str)
                    .collect::<Vec<_>>()
            })
            .collect();
        debug!(
            "get_user_roles: returning {:?} for user_id={}",
            roles, user_id
        );
        Ok(roles)
    }

    fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Try to get existing roles for this user
        let existing_roles: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse existing roles and check if this role is already present
            let mut roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(UserRole::from_str)
                .collect();

            if !roles.contains(&role) {
                roles.push(role);
                let roles_str = roles
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
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
                params![user_id, role.as_str()],
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
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse and filter out the role to remove
            let roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(UserRole::from_str)
                .filter(|r| r != &role)
                .collect();

            if roles.is_empty() {
                // No roles left, delete the row
                tx.execute(
                    &format!(
                        "DELETE FROM {} WHERE user_id = ?1",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![user_id],
                )?;
            } else {
                // Update with remaining roles
                let roles_str = roles
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
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
                    params![user_id, permission.as_int(), start_time_secs, end_time_secs, countdown_i64],
                )?;
                Ok(conn.last_insert_rowid() as usize)
            }
        }
    }

    fn remove_user_extra_permission(
        &self,
        permission_id: usize,
    ) -> Result<Option<(usize, Permission)>> {
        let conn = self.conn.lock().unwrap();

        // First, get the user_id and permission before deleting
        let result: Option<(usize, i32)> = conn
            .query_row(
                &format!(
                    "SELECT user_id, permission FROM {} WHERE id = ?1",
                    USER_EXTRA_PERMISSION_TABLE_V_4.name
                ),
                params![permission_id],
                |row| Ok((row.get::<_, usize>(0)?, row.get::<_, i32>(1)?)),
            )
            .ok();

        // Delete the permission
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
        )?;

        // Return the deleted info if found
        match result {
            Some((user_id, perm_int)) => {
                let permission = Permission::from_int(perm_int)
                    .ok_or_else(|| anyhow::anyhow!("Invalid permission int: {}", perm_int))?;
                Ok(Some((user_id, permission)))
            }
            None => Ok(None),
        }
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
        debug!(
            "resolve_user_permissions: user_id={} has roles: {:?}",
            user_id, roles
        );
        for role in &roles {
            let role_perms = role.permissions();
            debug!(
                "resolve_user_permissions: adding {:?} permissions from role {:?}",
                role_perms.len(),
                role
            );
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

        debug!(
            "resolve_user_permissions: checking extra permissions for user_id={} at timestamp={}",
            user_id, now
        );

        let mut stmt = conn.prepare(&format!(
            "SELECT permission FROM {} WHERE user_id = ?1 AND start_time <= ?2 AND (end_time IS NULL OR end_time >= ?2) AND (countdown IS NULL OR countdown > 0)",
            USER_EXTRA_PERMISSION_TABLE_V_4.name
        ))?;

        let extra_perms = stmt
            .query_map(params![user_id, now], |row| {
                let perm_int: i32 = row.get(0)?;
                Ok(perm_int)
            })?
            .filter_map(|r| r.ok().and_then(Permission::from_int))
            .collect::<Vec<_>>();

        debug!(
            "resolve_user_permissions: found {} extra permissions for user_id={}",
            extra_perms.len(),
            user_id
        );
        for perm in &extra_perms {
            debug!(
                "resolve_user_permissions: adding extra permission {:?}",
                perm
            );
            permissions.insert(*perm);
        }

        let final_permissions: Vec<Permission> = permissions.into_iter().collect();
        debug!(
            "resolve_user_permissions: final permissions for user_id={}: {:?}",
            user_id, final_permissions
        );
        Ok(final_permissions)
    }
}

fn system_time_from_column_result(value: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(value as u64)
}

const AUTH_TOKEN_ABSOLUTE_TTL_SECS: i64 = 30 * 24 * 60 * 60;
const AUTH_TOKEN_IDLE_TTL_SECS: i64 = 7 * 24 * 60 * 60;
const AUTH_TOKEN_ID_HEX_LEN: usize = 12;

fn auth_token_digest(value: &AuthTokenValue) -> String {
    let digest = Sha256::digest(value.0.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn auth_token_identifier(digest: &str) -> &str {
    &digest[..AUTH_TOKEN_ID_HEX_LEN]
}

fn unix_timestamp_now() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("System clock is before the Unix epoch")?
        .as_secs() as i64)
}

fn auth_token_is_expired(created: i64, last_used: Option<i64>, now: i64) -> bool {
    let absolute_cutoff = now.saturating_sub(AUTH_TOKEN_ABSOLUTE_TTL_SECS);
    let idle_cutoff = now.saturating_sub(AUTH_TOKEN_IDLE_TTL_SECS);
    created <= absolute_cutoff || last_used.unwrap_or(created) <= idle_cutoff
}

fn delete_expired_auth_tokens(conn: &Connection, now: i64) -> Result<usize> {
    let absolute_cutoff = now.saturating_sub(AUTH_TOKEN_ABSOLUTE_TTL_SECS);
    let idle_cutoff = now.saturating_sub(AUTH_TOKEN_IDLE_TTL_SECS);
    Ok(conn.execute(
        "DELETE FROM auth_token
         WHERE created <= ?1 OR COALESCE(last_used, created) <= ?2",
        params![absolute_cutoff, idle_cutoff],
    )?)
}

impl UserAuthTokenStore for SqliteUserStore {
    fn get_user_auth_token(&self, value: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let digest = auth_token_digest(value);
        let row = conn
            .query_row(
                "SELECT user_id, created, last_used, device_id
                 FROM auth_token WHERE token_hash = ?1",
                params![digest],
                |row| {
                    Ok((
                        row.get::<_, usize>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<usize>>(3)?,
                    ))
                },
            )
            .optional()?;

        let result = match row {
            Some((user_id, created, last_used, device_id)) => {
                if auth_token_is_expired(created, last_used, unix_timestamp_now()?) {
                    conn.execute(
                        "DELETE FROM auth_token WHERE token_hash = ?1",
                        params![digest],
                    )?;
                    Ok(None)
                } else {
                    Ok(Some(AuthToken {
                        user_id,
                        device_id,
                        value: value.clone(),
                        created: system_time_from_column_result(created),
                        last_used: last_used.map(system_time_from_column_result),
                    }))
                }
            }
            None => Ok(None),
        };
        record_db_query("get_user_auth_token", start.elapsed());
        result
    }

    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let digest = auth_token_digest(token);
        // Get the token data before deleting.
        let auth_token = match tx
            .prepare("SELECT user_id, created, last_used, device_id FROM auth_token WHERE token_hash = ?1")
            .and_then(|mut stmt| {
                stmt.query_row(params![digest], |row| {
                    Ok(AuthToken {
                        user_id: row.get(0)?,
                        device_id: row.get(3)?,
                        value: token.clone(),
                        created: system_time_from_column_result(row.get(1)?),
                        last_used: row
                            .get::<usize, Option<i64>>(2)?
                            .map(system_time_from_column_result),
                    })
                })
            }) {
                Ok(token) => token,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

        // Delete the token
        tx.execute(
            "DELETE FROM auth_token WHERE token_hash = ?1",
            params![digest],
        )?;

        tx.commit()?;
        Ok(Some(auth_token))
    }

    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = unix_timestamp_now()?;
        let digest = auth_token_digest(token);
        conn.execute(
            "UPDATE auth_token SET last_used = ?1 WHERE token_hash = ?2",
            params![now, digest],
        )?;
        Ok(())
    }

    fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let created = token
            .created
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let digest = auth_token_digest(&token.value);
        let token_id = auth_token_identifier(&digest);

        conn.execute(
            "INSERT INTO auth_token (user_id, token_hash, token_id, created, device_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token.user_id, digest, token_id, created, token.device_id],
        )?;
        record_db_query("add_user_auth_token", start.elapsed());
        Ok(())
    }

    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Result<Vec<AuthToken>> {
        let conn = self.conn.lock().unwrap();
        delete_expired_auth_tokens(&conn, unix_timestamp_now()?)?;
        let mut stmt = conn.prepare(
            "SELECT user_id, token_id, created, last_used, device_id
             FROM auth_token WHERE user_id = (SELECT id FROM user WHERE handle = ?1)",
        )?;
        let rows = stmt
            .query_map(params![user_handle], |row| {
                Ok(AuthToken {
                    user_id: row.get(0)?,
                    device_id: row.get(4)?,
                    value: AuthTokenValue(row.get(1)?),
                    created: system_time_from_column_result(row.get(2)?),
                    last_used: row
                        .get::<usize, Option<i64>>(3)?
                        .map(system_time_from_column_result),
                })
            })?
            .collect::<Result<Vec<AuthToken>, _>>()?;

        Ok(rows)
    }

    fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = unix_timestamp_now()?;
        let expired = delete_expired_auth_tokens(&conn, now)?;
        let cutoff_secs = now - (unused_for_days * 24 * 60 * 60) as i64;

        // Delete tokens that have never been used and are older than the cutoff
        // OR have been used but the last use is older than the cutoff
        let deleted = conn.execute(
            "DELETE FROM auth_token WHERE (last_used IS NULL AND created < ?1) OR (last_used IS NOT NULL AND last_used < ?1)",
            params![cutoff_secs],
        )?;

        Ok(expired + deleted)
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
        let mut stmt =
            conn.prepare("SELECT * FROM user_password_credentials WHERE user_id = ?1")?;

        let password_credentials = match stmt.query_row(params![user_id], |row| {
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
                    .map(system_time_from_column_result),
                last_used: row
                    .get::<usize, Option<i64>>(6)?
                    .map(system_time_from_column_result),
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
            params![
                user_id,
                date,
                endpoint_category,
                bytes_sent as i64,
                request_count as i64
            ],
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
            by_category: HashMap::new(),
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

    fn get_all_bandwidth_usage(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
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

    fn get_total_bandwidth_summary(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        let records = self.get_all_bandwidth_usage(start_date, end_date)?;

        let mut summary = BandwidthSummary {
            user_id: None,
            total_bytes_sent: 0,
            total_requests: 0,
            by_category: HashMap::new(),
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
                .unwrap_or_else(chrono::Utc::now);
            datetime
                .format("%Y%m%d")
                .to_string()
                .parse::<u32>()
                .unwrap_or(0)
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

        // If session_id is provided, check if it exists and belongs to this user
        // This enables clients to send progress updates and final data for the same session
        // while preventing one user from overwriting another user's session
        //
        // Returns (id, created) where:
        // - id: the row id of the event
        // - created: true only if this is a NEW finalized event (for metrics counting)
        //   This prevents double-counting when clients retry or update sessions
        let (id, created) = if let Some(ref session_id) = event.session_id {
            // Check if session exists and whether it was already finalized
            let existing: Option<(usize, usize, bool, String, u64)> = conn
                .query_row(
                    &format!(
                        "SELECT id, user_id, ended_at IS NOT NULL, track_id, started_at
                         FROM {} WHERE session_id = ?1",
                        LISTENING_EVENTS_TABLE_V_6.name
                    ),
                    params![session_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)? as usize,
                            row.get::<_, i64>(1)? as usize,
                            row.get(2)?,
                            row.get(3)?,
                            row.get::<_, i64>(4)? as u64,
                        ))
                    },
                )
                .optional()?;

            match existing {
                Some((id, existing_uid, _, _, _)) if existing_uid != event.user_id => {
                    // Session belongs to a different user - ignore this event
                    record_db_query("record_listening_event", start.elapsed());
                    return Ok((id, false));
                }
                Some((id, _, true, _, _)) => {
                    // Finalized events are immutable. Retries are idempotent and cannot
                    // rewrite trusted aggregate inputs.
                    (id, false)
                }
                Some((id, _, false, existing_track_id, existing_started_at))
                    if existing_track_id != event.track_id
                        || existing_started_at != event.started_at =>
                {
                    // An idempotency key cannot be repurposed for another playback.
                    (id, false)
                }
                Some((id, _, false, _, _)) => {
                    // Same user updating an in-progress session. Keep the stable row ID.
                    conn.execute(
                        &format!(
                            "UPDATE {} SET ended_at = ?1, duration_seconds = ?2,
                             track_duration_seconds = ?3, completed = ?4, seek_count = ?5,
                             pause_count = ?6, playback_context = ?7, client_type = ?8, date = ?9
                             WHERE id = ?10",
                            LISTENING_EVENTS_TABLE_V_6.name
                        ),
                        params![
                            event.ended_at.map(|t| t as i64),
                            event.duration_seconds as i64,
                            event.track_duration_seconds as i64,
                            if event.completed { 1 } else { 0 },
                            event.seek_count as i64,
                            event.pause_count as i64,
                            event.playback_context,
                            event.client_type,
                            event.date as i64,
                            id as i64,
                        ],
                    )?;
                    let created = event.ended_at.is_some();
                    (id, created)
                }
                None => {
                    // New session - insert
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
                    )?;
                    let id = conn.last_insert_rowid() as usize;
                    // New session, created = true only if finalized
                    let created = event.ended_at.is_some();
                    (id, created)
                }
            }
        } else {
            // No session_id, always insert as new event
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
            )?;
            let id = conn.last_insert_rowid() as usize;
            // No session_id means always new, created = true only if finalized
            let created = event.ended_at.is_some();
            (id, created)
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
                 FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
                 AND ended_at IS NOT NULL",
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
             FROM {} WHERE user_id = ?1 AND ended_at IS NOT NULL
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
                 FROM {} WHERE track_id = ?1 AND date >= ?2 AND date <= ?3
                 AND ended_at IS NOT NULL",
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
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
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
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
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

    fn get_all_track_play_counts(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<TrackPlayCount>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT track_id, COUNT(*) as play_count
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
             GROUP BY track_id",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let counts = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(TrackPlayCount {
                    track_id: row.get(0)?,
                    play_count: row.get::<_, i64>(1)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_all_track_play_counts", start.elapsed());
        Ok(counts)
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
                .unwrap_or_else(chrono::Utc::now);
            datetime
                .format("%Y%m%d")
                .to_string()
                .parse::<u32>()
                .unwrap_or(0)
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
                let setting =
                    UserSetting::from_key_value(key, &value).map_err(|e| anyhow::anyhow!(e))?;
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

        let mut stmt = conn
            .prepare("SELECT setting_key, setting_value FROM user_settings WHERE user_id = ?1")?;
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

    fn get_user_ids_with_setting(&self, key: &str, value: &str) -> Result<Vec<usize>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT user_id FROM user_settings WHERE setting_key = ?1 AND setting_value = ?2",
        )?;
        let rows = stmt.query_map(params![key, value], |row| row.get::<usize, usize>(0))?;

        let mut user_ids = Vec::new();
        for row in rows {
            user_ids.push(row?);
        }

        record_db_query("get_user_ids_with_setting", start.elapsed());
        Ok(user_ids)
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

    fn get_device(&self, device_id: usize) -> Result<Option<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE id = ?1",
            params![device_id],
            |row| {
                Ok(Device {
                    id: row.get(0)?,
                    device_uuid: row.get(1)?,
                    user_id: row.get(2)?,
                    device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                    device_name: row.get(4)?,
                    os_info: row.get(5)?,
                    first_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        record_db_query("get_device", start.elapsed());
        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_device_by_uuid(&self, device_uuid: &str) -> Result<Option<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE device_uuid = ?1",
            params![device_uuid],
            |row| {
                Ok(Device {
                    id: row.get(0)?,
                    device_uuid: row.get(1)?,
                    user_id: row.get(2)?,
                    device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                    device_name: row.get(4)?,
                    os_info: row.get(5)?,
                    first_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        record_db_query("get_device_by_uuid", start.elapsed());
        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE user_id = ?1 ORDER BY last_seen DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(Device {
                id: row.get(0)?,
                device_uuid: row.get(1)?,
                user_id: row.get(2)?,
                device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                device_name: row.get(4)?,
                os_info: row.get(5)?,
                first_seen: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                last_seen: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
            })
        })?;

        let devices: Result<Vec<Device>, _> = rows.collect();
        record_db_query("get_user_devices", start.elapsed());
        Ok(devices?)
    }

    fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE device SET user_id = ?1 WHERE id = ?2",
            params![user_id, device_id],
        )?;
        record_db_query("associate_device_with_user", start.elapsed());
        Ok(())
    }

    fn touch_device(&self, device_id: usize) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE device SET last_seen = ?1 WHERE id = ?2",
            params![now, device_id],
        )?;
        record_db_query("touch_device", start.elapsed());
        Ok(())
    }

    fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - (inactive_for_days as i64 * 24 * 60 * 60);

        let deleted = conn.execute(
            "DELETE FROM device WHERE user_id IS NULL AND last_seen < ?1",
            params![cutoff],
        )?;
        record_db_query("prune_orphaned_devices", start.elapsed());
        Ok(deleted)
    }

    fn prune_inactive_devices(&self, inactive_for_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - (inactive_for_days as i64 * 24 * 60 * 60);

        let deleted = conn.execute("DELETE FROM device WHERE last_seen < ?1", params![cutoff])?;
        record_db_query("prune_inactive_devices", start.elapsed());
        Ok(deleted)
    }

    fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Count current devices for user
        let device_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM device WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )?;

        if device_count <= max_devices {
            record_db_query("enforce_user_device_limit", start.elapsed());
            return Ok(0);
        }

        let to_delete = device_count - max_devices;

        // Delete oldest devices (by last_seen) beyond the limit
        let deleted = conn.execute(
            "DELETE FROM device WHERE id IN (
                SELECT id FROM device WHERE user_id = ?1
                ORDER BY last_seen ASC LIMIT ?2
            )",
            params![user_id, to_delete],
        )?;

        record_db_query("enforce_user_device_limit", start.elapsed());
        Ok(deleted)
    }

    fn get_device_share_policy(&self, device_id: usize) -> Result<DeviceSharePolicy> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let policy_row: Option<(String,)> = conn
            .query_row(
                "SELECT mode FROM device_share_policy WHERE device_id = ?1",
                params![device_id],
                |row| Ok((row.get(0)?,)),
            )
            .optional()?;

        let mode = match policy_row.map(|(m,)| m) {
            Some(m) => match m.as_str() {
                "allow_everyone" => DeviceShareMode::AllowEveryone,
                "deny_everyone" => DeviceShareMode::DenyEveryone,
                "custom" => DeviceShareMode::Custom,
                _ => DeviceShareMode::DenyEveryone,
            },
            None => {
                record_db_query("get_device_share_policy", start.elapsed());
                return Ok(DeviceSharePolicy::default());
            }
        };

        let mut allow_users = Vec::new();
        let mut allow_roles = Vec::new();
        let mut deny_users = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT rule_type, subject_type, subject_value
             FROM device_share_rule WHERE device_id = ?1",
        )?;
        let rows = stmt.query_map(params![device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (rule_type, subject_type, subject_value) = row?;
            match (rule_type.as_str(), subject_type.as_str()) {
                ("allow", "user_id") => {
                    if let Ok(id) = subject_value.parse::<usize>() {
                        allow_users.push(id);
                    }
                }
                ("allow", "role") => {
                    if let Some(role) = UserRole::from_str(&subject_value) {
                        allow_roles.push(role);
                    }
                }
                ("deny", "user_id") => {
                    if let Ok(id) = subject_value.parse::<usize>() {
                        deny_users.push(id);
                    }
                }
                _ => {}
            }
        }

        record_db_query("get_device_share_policy", start.elapsed());
        Ok(DeviceSharePolicy {
            mode,
            allow_users,
            allow_roles,
            deny_users,
        })
    }

    fn set_device_share_policy(&self, device_id: usize, policy: &DeviceSharePolicy) -> Result<()> {
        policy.validate()?;

        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mode_str = match policy.mode {
            DeviceShareMode::AllowEveryone => "allow_everyone",
            DeviceShareMode::DenyEveryone => "deny_everyone",
            DeviceShareMode::Custom => "custom",
        };

        conn.execute(
            "INSERT INTO device_share_policy (device_id, mode, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(device_id) DO UPDATE SET
                mode = ?2,
                updated_at = ?3",
            params![device_id, mode_str, now],
        )?;

        conn.execute(
            "DELETE FROM device_share_rule WHERE device_id = ?1",
            params![device_id],
        )?;

        if policy.mode == DeviceShareMode::Custom {
            for user_id in &policy.allow_users {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'allow', 'user_id', ?2, ?3)",
                    params![device_id, user_id.to_string(), now],
                )?;
            }
            for role in &policy.allow_roles {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'allow', 'role', ?2, ?3)",
                    params![device_id, role.as_str().to_lowercase(), now],
                )?;
            }
            for user_id in &policy.deny_users {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'deny', 'user_id', ?2, ?3)",
                    params![device_id, user_id.to_string(), now],
                )?;
            }
        }

        record_db_query("set_device_share_policy", start.elapsed());
        Ok(())
    }
}

impl user_store::UserEventStore for SqliteUserStore {
    fn append_event(
        &self,
        user_id: usize,
        event: &crate::user::sync_events::UserEvent,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        SqliteUserStore::append_event(self, user_id, event)
    }

    fn get_events_since(
        &self,
        user_id: usize,
        since_seq: i64,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        SqliteUserStore::get_events_since(self, user_id, since_seq)
    }

    fn get_current_seq(&self, user_id: usize) -> Result<i64> {
        SqliteUserStore::get_current_seq(self, user_id)
    }

    fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>> {
        SqliteUserStore::get_min_seq(self, user_id)
    }

    fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64> {
        SqliteUserStore::prune_events_older_than(self, before_timestamp)
    }

    fn set_user_role_with_event(
        &self,
        user_id: usize,
        role: UserRole,
        enabled: bool,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut roles: Vec<UserRole> = existing
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .filter_map(|value| UserRole::from_str(value.trim()))
            .collect();

        if enabled && !roles.contains(&role) {
            roles.push(role);
        } else if !enabled {
            roles.retain(|existing| existing != &role);
        }

        if roles.is_empty() {
            tx.execute(
                &format!(
                    "DELETE FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
            )?;
        } else {
            let roles = roles
                .iter()
                .map(|role| role.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if existing.is_some() {
                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![roles, user_id],
                )?;
            } else {
                tx.execute(
                    &format!(
                        "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![user_id, roles],
                )?;
            }
        }

        let event = UserEvent::PermissionsReset {
            permissions: resolve_permissions(&tx, user_id)?,
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn add_extra_permission_with_event(
        &self,
        user_id: usize,
        grant: PermissionGrant,
    ) -> Result<(usize, crate::user::sync_events::StoredEvent)> {
        let PermissionGrant::Extra {
            start_time,
            end_time,
            permission,
            countdown,
        } = grant
        else {
            bail!("Cannot add ByRole grant as extra permission");
        };

        let start_time = start_time.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;
        let end_time = end_time
            .map(|value| value.duration_since(SystemTime::UNIX_EPOCH))
            .transpose()?
            .map(|duration| duration.as_secs() as i64);
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO {} (user_id, permission, start_time, end_time, countdown)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![
                user_id,
                permission.as_int(),
                start_time,
                end_time,
                countdown.map(|value| value as i64)
            ],
        )?;
        let permission_id = tx.last_insert_rowid() as usize;
        let event = UserEvent::PermissionGranted { permission };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok((permission_id, stored))
    }

    fn remove_extra_permission_with_event(
        &self,
        permission_id: usize,
    ) -> Result<Option<(usize, Permission, crate::user::sync_events::StoredEvent)>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing: Option<(usize, i32)> = tx
            .query_row(
                &format!(
                    "SELECT user_id, permission FROM {} WHERE id = ?1",
                    USER_EXTRA_PERMISSION_TABLE_V_4.name
                ),
                params![permission_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((user_id, permission)) = existing else {
            tx.commit()?;
            return Ok(None);
        };
        let permission = Permission::from_int(permission)
            .ok_or_else(|| anyhow::anyhow!("Invalid permission int: {permission}"))?;
        tx.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
        )?;
        let event = UserEvent::PermissionRevoked { permission };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok(Some((user_id, permission, stored)))
    }

    fn set_liked_content_with_event(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
        operation_id: Option<&str>,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let event = if liked {
            crate::user::sync_events::UserEvent::ContentLiked {
                content_type,
                content_id: content_id.to_owned(),
            }
        } else {
            crate::user::sync_events::UserEvent::ContentUnliked {
                content_type,
                content_id: content_id.to_owned(),
            }
        };
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if existing.event != event {
                bail!("Operation id was already used for a different mutation");
            }
            tx.commit()?;
            return Ok(existing);
        }

        if liked {
            tx.execute(
                "INSERT OR IGNORE INTO liked_content (user_id, content_id, content_type)
                 VALUES (?1, ?2, ?3)",
                params![user_id, content_id, content_type.as_int()],
            )?;
        } else {
            tx.execute(
                "DELETE FROM liked_content WHERE user_id = ?1 AND content_id = ?2",
                params![user_id, content_id],
            )?;
        }
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn create_playlist_with_event(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_id: usize,
        track_ids: Vec<String>,
        operation_id: Option<&str>,
    ) -> Result<(String, crate::user::sync_events::StoredEvent)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if let crate::user::sync_events::UserEvent::PlaylistCreated { playlist_id, .. } =
                &existing.event
            {
                if !matches!(
                    &existing.event,
                    crate::user::sync_events::UserEvent::PlaylistCreated { name, .. }
                        if name == playlist_name
                ) {
                    return Err(super::UserServiceError::operation_conflict().into());
                }
                let playlist_id = playlist_id.clone();
                tx.commit()?;
                return Ok((playlist_id, existing));
            }
            return Err(super::UserServiceError::operation_conflict().into());
        }

        let mut playlist_id = random_string(16);
        while tx.query_row(
            "SELECT COUNT(*) FROM user_playlist WHERE id = ?1",
            params![playlist_id],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            playlist_id = random_string(16);
        }
        tx.execute(
            "INSERT INTO user_playlist (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
            params![&playlist_id, user_id, playlist_name, creator_id],
        )?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO user_playlist_tracks (playlist_id, track_id, position)
                 VALUES (?1, ?2, ?3)",
                params![&playlist_id, track_id, position as i32],
            )?;
        }
        let event = crate::user::sync_events::UserEvent::PlaylistCreated {
            playlist_id: playlist_id.clone(),
            name: playlist_name.to_owned(),
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok((playlist_id, stored))
    }

    fn update_playlist_with_events(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
        operation_id: Option<&str>,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing = Self::get_operation_events_tx(&tx, user_id, operation_id)?;
        if !existing.is_empty() {
            let expected: Vec<_> = playlist_name
                .iter()
                .map(
                    |name| crate::user::sync_events::UserEvent::PlaylistRenamed {
                        playlist_id: playlist_id.to_owned(),
                        name: name.clone(),
                    },
                )
                .chain(track_ids.iter().map(|tracks| {
                    crate::user::sync_events::UserEvent::PlaylistTracksUpdated {
                        playlist_id: playlist_id.to_owned(),
                        track_ids: tracks.clone(),
                    }
                }))
                .collect();
            if existing
                .iter()
                .map(|event| &event.event)
                .ne(expected.iter())
            {
                return Err(super::UserServiceError::operation_conflict().into());
            }
            tx.commit()?;
            return Ok(existing);
        }
        let owner: Option<usize> = tx
            .query_row(
                "SELECT user_id FROM user_playlist WHERE id = ?1",
                params![playlist_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(owner) = owner else {
            return Err(super::UserServiceError::playlist_not_found().into());
        };
        if owner != user_id {
            return Err(super::UserServiceError::playlist_not_found().into());
        }

        let mut events = Vec::new();
        if let Some(name) = playlist_name {
            tx.execute(
                "UPDATE user_playlist SET name = ?1 WHERE id = ?2",
                params![&name, playlist_id],
            )?;
            let event = crate::user::sync_events::UserEvent::PlaylistRenamed {
                playlist_id: playlist_id.to_owned(),
                name,
            };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        if let Some(track_ids) = track_ids {
            tx.execute(
                "DELETE FROM user_playlist_tracks WHERE playlist_id = ?1",
                params![playlist_id],
            )?;
            for (position, track_id) in track_ids.iter().enumerate() {
                tx.execute(
                    "INSERT INTO user_playlist_tracks (playlist_id, track_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![playlist_id, track_id, position as i32],
                )?;
            }
            let event = crate::user::sync_events::UserEvent::PlaylistTracksUpdated {
                playlist_id: playlist_id.to_owned(),
                track_ids,
            };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        tx.commit()?;
        Ok(events)
    }

    fn delete_playlist_with_event(
        &self,
        playlist_id: &str,
        user_id: usize,
        operation_id: Option<&str>,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if !matches!(
                &existing.event,
                crate::user::sync_events::UserEvent::PlaylistDeleted { playlist_id: id }
                    if id == playlist_id
            ) {
                return Err(super::UserServiceError::operation_conflict().into());
            }
            tx.commit()?;
            return Ok(existing);
        }
        let changed = tx.execute(
            "DELETE FROM user_playlist WHERE id = ?1 AND user_id = ?2",
            params![playlist_id, user_id],
        )?;
        if changed == 0 {
            return Err(super::UserServiceError::playlist_not_found().into());
        }
        let event = crate::user::sync_events::UserEvent::PlaylistDeleted {
            playlist_id: playlist_id.to_owned(),
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn set_settings_with_events(
        &self,
        user_id: usize,
        settings: Vec<UserSetting>,
        operation_id: Option<&str>,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing = Self::get_operation_events_tx(&tx, user_id, operation_id)?;
        if !existing.is_empty() {
            let expected: Vec<_> = settings
                .iter()
                .cloned()
                .map(|setting| crate::user::sync_events::UserEvent::SettingChanged { setting })
                .collect();
            if existing
                .iter()
                .map(|event| &event.event)
                .ne(expected.iter())
            {
                bail!("Operation id was already used for a different mutation");
            }
            tx.commit()?;
            return Ok(existing);
        }
        let mut events = Vec::with_capacity(settings.len());
        for setting in settings {
            tx.execute(
                "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
                 VALUES (?1, ?2, ?3, (cast(strftime('%s','now') as int)))
                 ON CONFLICT(user_id, setting_key) DO UPDATE SET
                    setting_value = excluded.setting_value, updated = excluded.updated",
                params![user_id, setting.key(), setting.value_to_string()],
            )?;
            let event = crate::user::sync_events::UserEvent::SettingChanged { setting };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        tx.commit()?;
        Ok(events)
    }

    fn get_sync_snapshot(
        &self,
        user_id: usize,
    ) -> Result<crate::user::sync_events::UserSyncSnapshot> {
        use std::collections::HashSet;

        fn liked(
            tx: &Transaction<'_>,
            user_id: usize,
            content_type: LikedContentType,
        ) -> Result<Vec<String>> {
            let mut stmt = tx.prepare(
                "SELECT content_id FROM liked_content
                 WHERE user_id = ?1 AND content_type = ?2",
            )?;
            let result = stmt
                .query_map(params![user_id, content_type.as_int()], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(result)
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let seq = tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM user_events WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )?;
        let liked_albums = liked(&tx, user_id, LikedContentType::Album)?;
        let liked_artists = liked(&tx, user_id, LikedContentType::Artist)?;
        let liked_tracks = liked(&tx, user_id, LikedContentType::Track)?;

        let settings = {
            let mut stmt = tx.prepare(
                "SELECT setting_key, setting_value FROM user_settings WHERE user_id = ?1",
            )?;
            let result = stmt
                .query_map(params![user_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|row| row.ok())
                .filter_map(|(key, value)| UserSetting::from_key_value(&key, &value).ok())
                .collect();
            result
        };

        let playlists = {
            let mut stmt = tx.prepare(
                "SELECT p.id, p.user_id, u.handle, p.name, p.created
                 FROM user_playlist p
                 JOIN user u ON u.id = p.creator_id
                 WHERE p.user_id = ?1",
            )?;
            let rows = stmt
                .query_map(params![user_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, usize>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut playlists = Vec::with_capacity(rows.len());
            for (id, owner_id, creator, name, created) in rows {
                let mut tracks_stmt = tx.prepare(
                    "SELECT track_id FROM user_playlist_tracks
                     WHERE playlist_id = ?1 ORDER BY position ASC",
                )?;
                let tracks = tracks_stmt
                    .query_map(params![&id], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                playlists.push(UserPlaylist {
                    id,
                    user_id: owner_id,
                    creator,
                    name,
                    created: system_time_from_column_result(created),
                    tracks,
                });
            }
            playlists
        };

        let mut permissions = HashSet::new();
        let roles: Option<String> = tx
            .query_row(
                "SELECT role FROM user_role WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(roles) = roles {
            for role in roles
                .split(',')
                .filter_map(|role| UserRole::from_str(role.trim()))
            {
                permissions.extend(role.permissions().iter().copied());
            }
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs() as i64;
        {
            let mut stmt = tx.prepare(
                "SELECT permission FROM user_extra_permission
                 WHERE user_id = ?1 AND start_time <= ?2
                   AND (end_time IS NULL OR end_time >= ?2)
                   AND (countdown IS NULL OR countdown > 0)",
            )?;
            for value in stmt
                .query_map(params![user_id, now], |row| row.get::<_, i32>(0))?
                .filter_map(|value| value.ok())
            {
                if let Some(permission) = Permission::from_int(value) {
                    permissions.insert(permission);
                }
            }
        }

        let notifications = {
            let mut stmt = tx.prepare(
                "SELECT id, notification_type, title, body, data, read_at, created_at
                 FROM user_notifications WHERE user_id = ?1
                 ORDER BY created_at DESC, rowid DESC",
            )?;
            let raw = stmt
                .query_map(params![user_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            raw.into_iter()
                .map(|(id, kind, title, body, data, read_at, created_at)| {
                    Ok(crate::notifications::Notification {
                        id,
                        notification_type: serde_json::from_str(&kind)?,
                        title,
                        body,
                        data: serde_json::from_str(&data)?,
                        read_at,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>>>()?
        };

        tx.commit()?;
        Ok(crate::user::sync_events::UserSyncSnapshot {
            seq,
            liked_albums,
            liked_artists,
            liked_tracks,
            settings,
            playlists,
            permissions: permissions.into_iter().collect(),
            notifications,
        })
    }
}

fn resolve_permissions(conn: &Connection, user_id: usize) -> Result<Vec<Permission>> {
    let mut permissions = HashSet::new();
    let roles: Option<String> = conn
        .query_row(
            &format!(
                "SELECT role FROM {} WHERE user_id = ?1",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(roles) = roles {
        for role in roles
            .split(',')
            .filter_map(|value| UserRole::from_str(value.trim()))
        {
            permissions.extend(role.permissions().iter().copied());
        }
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as i64;
    let mut stmt = conn.prepare(&format!(
        "SELECT permission FROM {} WHERE user_id = ?1 AND start_time <= ?2
         AND (end_time IS NULL OR end_time >= ?2)
         AND (countdown IS NULL OR countdown > 0)",
        USER_EXTRA_PERMISSION_TABLE_V_4.name
    ))?;
    for value in stmt
        .query_map(params![user_id, now], |row| row.get::<_, i32>(0))?
        .filter_map(|value| value.ok())
    {
        if let Some(permission) = Permission::from_int(value) {
            permissions.insert(permission);
        }
    }
    Ok(permissions.into_iter().collect())
}

impl crate::notifications::NotificationStore for SqliteUserStore {
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: crate::notifications::NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<crate::notifications::Notification> {
        let start = Instant::now();
        let id = format!("notif_{}", random_string(16));
        let created_at = chrono::Utc::now().timestamp();
        let type_str = serde_json::to_string(&notification_type)?;
        let data_str = serde_json::to_string(&data)?;

        let conn = self.conn.lock().unwrap();

        // Insert the notification
        conn.execute(
            "INSERT INTO user_notifications (id, user_id, notification_type, title, body, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, user_id, type_str, title, body, data_str, created_at],
        )?;

        // Enforce 100-per-user limit: delete oldest beyond limit
        // Use rowid as tiebreaker when timestamps are equal (e.g., rapid inserts)
        conn.execute(
            "DELETE FROM user_notifications WHERE user_id = ?1 AND id NOT IN (
                SELECT id FROM user_notifications WHERE user_id = ?1
                ORDER BY created_at DESC, rowid DESC LIMIT 100
            )",
            params![user_id],
        )?;

        record_db_query("create_notification", start.elapsed());

        Ok(crate::notifications::Notification {
            id,
            notification_type,
            title,
            body,
            data,
            read_at: None,
            created_at,
        })
    }

    fn get_user_notifications(
        &self,
        user_id: usize,
    ) -> Result<Vec<crate::notifications::Notification>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, notification_type, title, body, data, read_at, created_at
             FROM user_notifications
             WHERE user_id = ?1
             ORDER BY created_at DESC, rowid DESC",
        )?;

        let notifications = stmt
            .query_map(params![user_id], |row| {
                let id: String = row.get(0)?;
                let type_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let body: Option<String> = row.get(3)?;
                let data_str: String = row.get(4)?;
                let read_at: Option<i64> = row.get(5)?;
                let created_at: i64 = row.get(6)?;

                Ok((id, type_str, title, body, data_str, read_at, created_at))
            })?
            .filter_map(|r| r.ok())
            .filter_map(
                |(id, type_str, title, body, data_str, read_at, created_at)| {
                    let notification_type: crate::notifications::NotificationType =
                        serde_json::from_str(&type_str).ok()?;
                    let data: serde_json::Value = serde_json::from_str(&data_str).ok()?;

                    Some(crate::notifications::Notification {
                        id,
                        notification_type,
                        title,
                        body,
                        data,
                        read_at,
                        created_at,
                    })
                },
            )
            .collect();

        record_db_query("get_user_notifications", start.elapsed());
        Ok(notifications)
    }

    fn get_notification(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, notification_type, title, body, data, read_at, created_at
                 FROM user_notifications
                 WHERE id = ?1 AND user_id = ?2",
                params![notification_id, user_id],
                |row| {
                    let id: String = row.get(0)?;
                    let type_str: String = row.get(1)?;
                    let title: String = row.get(2)?;
                    let body: Option<String> = row.get(3)?;
                    let data_str: String = row.get(4)?;
                    let read_at: Option<i64> = row.get(5)?;
                    let created_at: i64 = row.get(6)?;
                    Ok((id, type_str, title, body, data_str, read_at, created_at))
                },
            )
            .optional()?;

        record_db_query("get_notification", start.elapsed());

        match result {
            Some((id, type_str, title, body, data_str, read_at, created_at)) => {
                let notification_type: crate::notifications::NotificationType =
                    serde_json::from_str(&type_str)?;
                let data: serde_json::Value = serde_json::from_str(&data_str)?;

                Ok(Some(crate::notifications::Notification {
                    id,
                    notification_type,
                    title,
                    body,
                    data,
                    read_at,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    fn mark_notification_read(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        let start = Instant::now();
        let read_at = chrono::Utc::now().timestamp();

        let conn = self.conn.lock().unwrap();

        let rows_affected = conn.execute(
            "UPDATE user_notifications SET read_at = ?1 WHERE id = ?2 AND user_id = ?3 AND read_at IS NULL",
            params![read_at, notification_id, user_id],
        )?;

        record_db_query("mark_notification_read", start.elapsed());

        if rows_affected == 0 {
            // Either doesn't exist, doesn't belong to user, or already read
            // Try to fetch it to check if it exists and belongs to user
            drop(conn);
            return self.get_notification(notification_id, user_id);
        }

        // Fetch and return the updated notification
        drop(conn);
        self.get_notification(notification_id, user_id)
    }

    fn get_unread_count(&self, user_id: usize) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = ?1 AND read_at IS NULL",
            params![user_id],
            |row| row.get(0),
        )?;

        record_db_query("get_unread_count", start.elapsed());
        Ok(count as usize)
    }
}

include!("sqlite_user_store_tests.rs");
