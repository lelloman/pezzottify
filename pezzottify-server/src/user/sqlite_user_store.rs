#![allow(dead_code)]

use crate::server::metrics::record_db_query;
use crate::sqlite_column;
use crate::sqlite_persistence::{
    configure_connection, Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema,
    BASE_DB_VERSION, DEFAULT_TIMESTAMP,
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
        configure_connection(&conn)?;

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

include!("sqlite_user_core.rs");
include!("sqlite_auth.rs");
include!("sqlite_bandwidth.rs");
include!("sqlite_listening.rs");
include!("sqlite_settings.rs");
include!("sqlite_devices.rs");
include!("sqlite_events.rs");
include!("sqlite_notifications.rs");
