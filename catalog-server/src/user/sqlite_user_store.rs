use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema, BASE_DB_VERSION,
    DEFAULT_TIMESTAMP,
};
use crate::user::*;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info};
use tracing_subscriber::field::debug;

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
        let conn = if db_path.as_ref().exists() {
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

        Self::migrate_if_needed(&conn, version)?;

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

    fn migrate_if_needed(conn: &Connection, version: usize) -> Result<()> {
        let mut latest_from = version;
        for schema in VERSIONED_SCHEMAS.iter().skip(version + 1) {
            if let Some(migration_fn) = schema.migration {
                info!(
                    "Migrating db from version {} to {}",
                    latest_from, schema.version
                );
                migration_fn(conn)?;
                latest_from = schema.version;
            }
        }
        conn.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + latest_from),
            [],
        )?;

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
        .with_context(|| format!("Failed to create user {}", user_handle))
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

    fn get_user_handle(&self, user_id: usize) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT handle FROM {} WHERE id = ?1",
                USER_TABLE_V_0.name
            ))
            .ok()?;
        let handle: String = stmt.query_row(params![user_id], |row| row.get(0)).ok()?;

        Some(handle)
    }

    fn get_all_user_handles(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT handle FROM {}", USER_TABLE_V_0.name))
            .ok()
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get(0))
            .ok()
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .ok()
            .unwrap();

        rows
    }

    fn get_user_id(&self, user_handle: &str) -> Option<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM {} WHERE handle = ?1",
                USER_TABLE_V_0.name
            ))
            .ok()?;
        let id: i32 = stmt
            .query_row(params![user_handle], |row| row.get(0))
            .ok()?;

        Some(id as usize)
    }

    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Option<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT COUNT(*) FROM {} WHERE user_id = ?1 AND content_id = ?2",
                LIKED_CONTENT_TABLE_V_2.name
            ))
            .ok()?;
        let count: i32 = stmt
            .query_row(params![user_id, content_id], |row| row.get(0))
            .ok()?;

        Some(count > 0)
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
                    "INSERT INTO {} (user_id, content_id, content_type) VALUES (?1, ?2, ?3)",
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT role FROM {} WHERE user_id = ?1",
            USER_ROLE_TABLE_V_4.name
        ))?;
        let roles = stmt
            .query_map(params![user_id], |row| {
                let role_str: String = row.get(0)?;
                Ok(role_str)
            })?
            .filter_map(|r| r.ok().and_then(|s| UserRole::from_str(&s)))
            .collect();
        Ok(roles)
    }

    fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO {} (user_id, role) VALUES (?1, ?2)",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id, role.to_string()],
        )?;
        Ok(())
    }

    fn remove_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE user_id = ?1 AND role = ?2",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id, role.to_string()],
        )?;
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
                let end_time_secs = end_time.map(|t| {
                    t.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64
                });
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
        let conn = self.conn.lock().unwrap();

        // Get current countdown
        let current_countdown: Option<i64> = conn.query_row(
            &format!(
                "SELECT countdown FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
            |row| row.get(0),
        )?;

        match current_countdown {
            None => Ok(true), // No countdown, permission remains valid
            Some(count) if count <= 1 => {
                // Last use, delete the permission
                self.remove_user_extra_permission(permission_id)?;
                Ok(false)
            }
            Some(count) => {
                // Decrement the countdown
                conn.execute(
                    &format!(
                        "UPDATE {} SET countdown = ?1 WHERE id = ?2",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![count - 1, permission_id],
                )?;
                Ok(true)
            }
        }
    }

    fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>> {
        use std::collections::HashSet;

        let mut permissions = HashSet::new();

        // Add permissions from roles
        let roles = self.get_user_roles(user_id)?;
        for role in roles {
            for permission in role.permissions() {
                permissions.insert(*permission);
            }
        }

        // Add active extra permissions
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

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

        for perm in extra_perms {
            permissions.insert(perm);
        }

        Ok(permissions.into_iter().collect())
    }
}

fn system_time_from_column_result(value: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(value as u64)
}

impl UserAuthTokenStore for SqliteUserStore {
    fn get_user_auth_token(&self, value: &AuthTokenValue) -> Option<AuthToken> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM auth_token WHERE value = ?1")
            .ok()?;
        stmt.query_row(params![value.0], |row| {
            Ok(AuthToken {
                user_id: row.get(0)?,
                value: AuthTokenValue(row.get(1)?),
                created: system_time_from_column_result(row.get(2)?),
                last_used: row
                    .get::<usize, Option<i64>>(3)?
                    .map(|v| system_time_from_column_result(v)),
            })
        })
        .ok()
    }

    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Option<AuthToken> {
        let token = self.get_user_auth_token(token)?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("DELETE FROM auth_token WHERE value = ?1")
            .ok()
            .unwrap();
        match stmt.execute(params![token.value.0]) {
            Ok(_) => Some(token),
            Err(_) => None,
        }
    }

    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("UPDATE auth_token SET last_used = CURRENT_TIMESTAMP WHERE value = ?1")
            .ok()
            .unwrap();
        let _ = stmt.execute(params![token.0])?;
        Ok(())
    }

    fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO auth_token (value, user_id) VALUES (?1, ?2)",
            params![token.value.0, token.user_id,],
        )?;
        Ok(())
    }

    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Vec<AuthToken> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT * FROM auth_token WHERE user_id = (SELECT id FROM user WHERE handle = ?1)",
            )
            .ok()
            .unwrap();
        let rows = stmt
            .query_map(params![user_handle], |row| {
                Ok(AuthToken {
                    user_id: row.get(0)?,
                    value: AuthTokenValue(row.get(1)?),
                    created: system_time_from_column_result(row.get(2)?),
                    last_used: row
                        .get::<usize, Option<i64>>(3)?
                        .map(|v| system_time_from_column_result(v)),
                })
            })
            .ok()
            .unwrap()
            .collect::<Result<Vec<AuthToken>, _>>()
            .ok()
            .unwrap();

        rows
    }
}

impl UserAuthCredentialsStore for SqliteUserStore {
    fn get_user_auth_credentials(&self, user_handle: &str) -> Option<UserAuthCredentials> {
        let user_id = self.get_user_id(user_handle)?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM user_password_credentials WHERE user_id = ?1")
            .ok()?;

        let password_credentials = stmt
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
            })
            .ok();

        Some(UserAuthCredentials {
            user_id,
            username_password: password_credentials,
            keys: vec![],
        })
    }

    fn update_user_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let user_id = credentials.user_id;
        match credentials.username_password.as_ref() {
            Some(password_credentials) => {
                let updated = conn.execute(
                    "UPDATE user_password_credentials SET salt = ?1, hash = ?2, hasher = ?3, user_id = ?4",
                    params![
                        password_credentials.salt,
                        password_credentials.hash,
                        password_credentials.hasher.to_string(),
                        user_id
                    ],
                )?;
                if updated == 0 {
                    conn.execute(
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
                conn.execute(
                    "DELETE FROM user_password_credentials WHERE user_id = ?1",
                    params![user_id],
                )?;
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
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
            .unwrap());

        store
            .set_user_liked_content(test_user_id, "test_content", LikedContentType::Album, false)
            .unwrap();

        assert!(!store
            .is_user_liked_content(test_user_id, "test_content")
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
            conn.execute("INSERT INTO user (handle) VALUES (?1)", params!["test_user"])
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

        // Now open with SqliteUserStore, which should trigger migration to V4
        let store = SqliteUserStore::new(&temp_file_path).unwrap();

        // Verify we're now at V4
        {
            let conn = store.conn.lock().unwrap();
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(db_version, BASE_DB_VERSION as i64 + 4);

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
        }

        // Verify old data is still intact
        let user_id = store.get_user_id("test_user").unwrap();
        assert_eq!(user_id, 1);

        let liked_content = store
            .is_user_liked_content(user_id, "test_content_id")
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
}
