use anyhow::Result;
use hyper::Version;
use rusqlite::Connection;

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

pub struct VersionedSchema {
    pub version: usize,
    pub tables: &'static [Table],
    pub create: fn(&Connection, &VersionedSchema) -> Result<()>,
    pub migration: Option<fn(&Connection) -> Result<()>>,
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
    },
];
