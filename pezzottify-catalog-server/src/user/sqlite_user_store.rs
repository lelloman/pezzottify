use crate::sqlite_persistence::{Table, VersionedSchema, BASE_DB_VERSION};
use crate::user::*;
use crate::user::{auth::PezzottifyHasher, user_models::LikedContentType};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::info;

/// V 0
const USER_TABLE_V_0: Table = Table {
    name: "user",
    schema: "CREATE TABLE user (id INTEGER UNIQUE, handle TEXT NOT NULL UNIQUE, created INTEGER DEFAULT (cast(strftime('%s','now') as int)), PRIMARY KEY (id));",
    indices: &["CREATE INDEX handle_index ON user (handle);"],
};
const LIKED_CONTENT_TABLE_V_0: Table = Table {
    name: "liked_content",
    schema: "CREATE TABLE liked_content (id INTEGER NOT NULL UNIQUE, user_id TEXT NOT NULL, content_id TEXT NOT NULL, created INTEGER DEFAULT (cast(strftime('%s','now') as int)), PRIMARY KEY (id), CONSTRAINT user_id FOREIGN KEY (user_id) REFERENCES user (id));",
    indices: &[],
};
const AUTH_TOKEN_TABLE_V_0: Table = Table {
    name: "auth_token",
    schema: "CREATE TABLE auth_token (user_id INTEGER NOT NULL, value TEXT NOT NULL UNIQUE, created INTEGER DEFAULT (cast(strftime('%s','now') as int)), last_used INTEGER)",
    indices: &["CREATE INDEX auth_token_value_index ON auth_token (value);"],
};
const USER_PASSWORD_CREDENTIALS_V_0: Table = Table {
    name: "user_password_credentials",
    schema: "CREATE TABLE user_password_credentials (user_id INTEGER NOT NULL, salt TEXT NOT NULL, hash	INTEGER NOT NULL, hasher TEXT NOT NULL, created	INTEGER DEFAULT (cast(strftime('%s','now') as int)), last_tried INTEGER, last_used INTEGER, CONSTRAINT user_id FOREIGN KEY(user_id) REFERENCES user(id) ON DELETE CASCADE)",
    indices: &[],
};

fn create_v0(conn: &Connection, schema: &VersionedSchema) -> Result<()> {
    conn.execute("PRAGMA foreign_keys = ON;", [])?;
    for table in schema.tables {
        conn.execute(table.schema, [])?;
        for index in table.indices {
            conn.execute(index, [])?;
        }
    }
    conn.execute(
        &format!("PRAGMA user_version = {}", BASE_DB_VERSION + schema.version),
        [],
    )?;
    Ok(())
}

/// V 1
const LIKED_CONTENT_TABLE_V_1: Table = Table {
    name: "liked_content",
    schema: "CREATE TABLE liked_content (id INTEGER NOT NULL UNIQUE, user_id TEXT NOT NULL, content_id TEXT NOT NULL, content_type INTEGER NOT NULL, created INTEGER DEFAULT (cast(strftime('%s','now') as int)), PRIMARY KEY (id), CONSTRAINT user_id FOREIGN KEY (user_id) REFERENCES user (id));",
    indices: &[],
};
fn validate_schema_1(conn: &Connection) -> Result<()> {
    // Verify user table column names
    let mut stmt = conn.prepare("PRAGMA table_info(user);")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;

    if columns != ["id", "handle", "created"] {
        bail!(
            "Schema validation failed for user table. found {:?}",
            columns
        );
    }

    // Verify liked_content table column names
    let mut stmt = conn.prepare("PRAGMA table_info(liked_content);")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;

    if columns.len() != 5 {
        bail!("Schema validation failed for linked_content table, should have 5 columns but actually has {}.", columns.len());
    }
    for name in ["id", "user_id", "content_id", "content_type", "created"] {
        if !columns.contains(&name.to_string()) {
            bail!(
                "Schema validation failed for linked_content table, missing {} column.",
                name
            );
        }
    }

    Ok(())
}

/// V 2
const LIKED_CONTENT_TABLE_V_2: Table = Table {
    name: "liked_content",
    schema: "CREATE TABLE liked_content (id INTEGER NOT NULL UNIQUE, user_id INTEGER NOT NULL, content_id TEXT NOT NULL, content_type INTEGER NOT NULL, created INTEGER DEFAULT (cast(strftime('%s','now') as int)), UNIQUE(user_id, content_id), PRIMARY KEY (id), CONSTRAINT user_id FOREIGN KEY (user_id) REFERENCES user (id));",
    indices: &[],
};

/// V 3
const USER_PLAYLIST_TABLE_V_3: Table = Table {
    name: "user_playlist",
    schema: "CREATE TABLE user_playlist (id INTEGER, user_id INTEGER NOT NULL, name	TEXT, created INTEGER DEFAULT (CAST(strftime('%s', 'now') AS int)),	PRIMARY KEY(id), CONSTRAINT user_id FOREIGN KEY(user_id) REFERENCES user(id) ON DELETE CASCADE)",
    indices: &[],
};
const USER_PLAYLIST_TRACKS_TABLE_V_3: Table = Table {
    name: "user_playlist_tracks",
    schema: "CREATE TABLE user_playlist_tracks (id INTEGER, track_id TEXT NOT NULL, playlist_id INTEGER NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(id), CONSTRAINT playlist_id FOREIGN KEY(playlist_id) REFERENCES user_playlist(id) ON DELETE CASCADE)",
    indices: &[],
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
        create: create_v0,
        migration: None,
        validate: |conn: &Connection| Ok(()),
    },
    VersionedSchema {
        version: 1,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_1,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
        ],
        create: create_v0,
        migration: Some(|conn: &Connection| {
            conn.execute(
                "ALTER TABLE liked_content ADD COLUMN content_type INTEGER NOT NULL DEFAULT 1000",
                [],
            )?;
            Ok(())
        }),
        validate: validate_schema_1,
    },
    VersionedSchema {
        version: 2,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
        ],
        create: create_v0,
        migration: Some(|conn: &Connection| {
            // Rename liked_content to liked_content_backup
            conn.execute(
                "ALTER TABLE liked_content RENAME TO liked_content_backup;",
                [],
            )?;

            // Create the new liked_content table
            conn.execute(&LIKED_CONTENT_TABLE_V_2.schema, [])?;

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
        validate: validate_schema_1,
    },
    VersionedSchema {
        version: 3,
        tables: &[
            USER_TABLE_V_0,
            LIKED_CONTENT_TABLE_V_2,
            AUTH_TOKEN_TABLE_V_0,
            USER_PASSWORD_CREDENTIALS_V_0,
            USER_PLAYLIST_TABLE_V_3,
        ],
        create: create_v0,
        migration: Some(|conn: &Connection| {
            conn.execute(&USER_PLAYLIST_TABLE_V_3.schema, [])?;
            conn.execute(&USER_PLAYLIST_TRACKS_TABLE_V_3.schema, [])?;
            Ok(())
        }),
        validate: validate_schema_1,
    },
];

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
            Self::create_schema(&conn)?;
            conn
        };

        // Read the database version
        let version = conn
            .query_row("PRAGMA user_version;", [], |row| row.get::<usize, usize>(0))
            .context("Failed to read database version")?
            - BASE_DB_VERSION;

        if version >= VERSIONED_SCHEMAS.len() {
            bail!("Database version {} is too new", version);
        } else {
            (VERSIONED_SCHEMAS
                .get(version)
                .context("Failed to get schema")?
                .validate)(&conn)?;
        }

        Self::migrate_if_needed(&conn, version)?;

        Ok(SqliteUserStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn infer_path() -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;

        loop {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    if path.is_file() {
                        if let Some(s) = path.file_name() {
                            if s.to_string_lossy() == "pezzottify_store.db" {
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

    fn create_schema(conn: &Connection) -> Result<()> {
        let latest_version = VERSIONED_SCHEMAS.last().unwrap();
        let create_fn = latest_version.create;
        create_fn(conn, latest_version)
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

    fn get_user_playlists(
        &self,
        user_id: &str,
    ) -> Option<Vec<crate::user::user_models::UserPlaylist>> {
        todo!()
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
                    value: AuthTokenValue(row.get(0)?),
                    user_id: row.get(1)?,
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
}
