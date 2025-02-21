use super::VERSIONED_SCHEMAS;
use crate::user::auth::PezzottifyHasher;
use crate::user::*;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone)]
pub struct SqliteUserStore {
    conn: Arc<Mutex<Connection>>,
}

const BASE_DB_VERSION: u32 = 199;

const TABLE_USER: &str = "user";
const TABLE_LIKED_CONTENT: &str = "liked_content";

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
        let version: i32 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .context("Failed to read database version")?;

        match version {
            199 => Self::validate_schema_0(&conn)?,
            _ => bail!("Unknown database version {}", version),
        }

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
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        let last_versioned_schema = VERSIONED_SCHEMAS.last().unwrap();
        for table in last_versioned_schema.tables {
            conn.execute(&table.schema, [])?;
            for index in table.indices {
                conn.execute(index, [])?;
            }
        }

        conn.execute(
            &format!(
                "PRAGMA user_version = {}",
                BASE_DB_VERSION + last_versioned_schema.version
            ),
            [],
        )?;

        Ok(())
    }

    fn validate_schema_0(conn: &Connection) -> Result<()> {
        // Verify user table column names
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({});", TABLE_USER))?;
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
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({});", TABLE_LIKED_CONTENT))?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<Result<_, _>>()?;

        if columns != ["id", "user_id", "content_id", "created"] {
            bail!("Schema validation failed for linked_content table.");
        }

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
            .prepare(&format!("SELECT handle FROM {} WHERE id = ?1", TABLE_USER))
            .ok()?;
        let handle: String = stmt.query_row(params![user_id], |row| row.get(0)).ok()?;

        Some(handle)
    }

    fn get_all_user_handles(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT handle FROM {}", TABLE_USER))
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
            .prepare(&format!("SELECT id FROM {} WHERE handle = ?1", TABLE_USER))
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
                TABLE_LIKED_CONTENT
            ))
            .ok()?;
        let count: i32 = stmt
            .query_row(params![user_id, content_id], |row| row.get(0))
            .ok()?;

        Some(count > 0)
    }

    fn set_user_liked_content(&self, user_id: usize, content_id: &str, liked: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if liked {
            conn.execute(
                &format!(
                    "INSERT INTO {} (user_id, content_id) VALUES (?1, ?2)",
                    TABLE_LIKED_CONTENT
                ),
                params![user_id, content_id],
            )?;
        } else {
            conn.execute(
                &format!(
                    "DELETE FROM {} WHERE user_id = ?1 AND content_id = ?2",
                    TABLE_LIKED_CONTENT
                ),
                params![user_id, content_id],
            )?;
        }

        Ok(())
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

        let result = store.set_user_liked_content(1, "test_content", true);
        assert!(result.is_err());
    }

    #[test]
    fn creates_liked_content() {
        let (store, _temp_dir) = create_tmp_store();

        let test_user_id = store.create_user("test_user").unwrap();
        store
            .set_user_liked_content(test_user_id, "test_content", true)
            .unwrap();

        assert!(store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap());

        store
            .set_user_liked_content(test_user_id, "test_content", false)
            .unwrap();

        assert!(!store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap());
    }
}
