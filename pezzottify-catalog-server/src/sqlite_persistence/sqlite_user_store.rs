use crate::user::{User, UserPlaylist, UserSessionView, UserStore};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::sync::Mutex;
use std::{collections::HashMap, path::Path};

pub struct SqliteUserStore {
    conn: Mutex<Connection>,
}

const BASE_DB_VERSION: i32 = 199;
const CURRENT_DB_VERSION: i32 = 0;

const TABLE_USER: &str = "user";
const TABLE_LIKED_CONTENT: &str = "liked_content";

const VERSION_1_SCHEMA_TABLES: &[&str] = &[
    "CREATE TABLE user (id INTEGER UNIQUE, handle TEXT NOT NULL UNIQUE, PRIMARY KEY (id));",
    "CREATE TABLE liked_content (id INTEGER NOT NULL UNIQUE, user_id TEXT NOT NULL, content_id TEXT NOT NULL, created INTEGER DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (id), CONSTRAINT user_id FOREIGN KEY (user_id) REFERENCES user (id));",
    "CREATE INDEX handle_index ON user (handle);",
];

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
            conn: Mutex::new(conn),
        })
    }

    fn create_schema(conn: &Connection) -> Result<()> {
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        for table in VERSION_1_SCHEMA_TABLES {
            let table_sql = table
                .replace("user", TABLE_USER)
                .replace("liked_content", TABLE_LIKED_CONTENT);
            conn.execute(&table_sql, [])?;
        }
        conn.execute(
            &format!(
                "PRAGMA user_version = {}",
                BASE_DB_VERSION + CURRENT_DB_VERSION
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

        if columns != ["id", "handle"] {
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
