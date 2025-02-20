pub struct Table {
    pub name: &'static str,
    pub schema: &'static str,
    pub indices: &'static [&'static str],
}

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

pub struct VersionedSchema {
    pub version: u32,
    pub tables: &'static [Table],
}

pub const VERSIONED_SCHEMAS: &[VersionedSchema] = &[VersionedSchema {
    version: 0,
    tables: &[
        USER_TABLE_V_0,
        LIKED_CONTENT_TABLE_V_0,
        AUTH_TOKEN_TABLE_V_0,
        USER_PASSWORD_CREDENTIALS_V_0,
    ],
}];
