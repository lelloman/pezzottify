use anyhow::{bail, Result};
use hyper::Version;
use rusqlite::{params, Connection};

pub struct Table {
    pub name: &'static str,
    pub schema: &'static str,
    pub indices: &'static [&'static str],
}

pub const BASE_DB_VERSION: usize = 99999;

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

pub struct VersionedSchema {
    pub version: usize,
    pub tables: &'static [Table],
    pub create: fn(&Connection, &VersionedSchema) -> Result<()>,
    pub migration: Option<fn(&Connection) -> Result<()>>,
    pub validate: fn(&Connection) -> Result<()>,
}

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
];
