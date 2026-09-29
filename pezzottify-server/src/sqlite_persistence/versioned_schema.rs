use anyhow::Result;
use rusqlite::{params, Connection};
use simple_server::database::migrations::{MigrationEntry, MigrationPlan, MigrationVersion};

pub const DEFAULT_TIMESTAMP: &str = "(cast(strftime('%s','now') as int))";

#[macro_export]
macro_rules! sqlite_column {
    ($name:expr, $sql_type:expr $(, $field:ident = $value:expr)*) => {
        {
            // Allow unused_mut because the variable is only mutated when optional
            // field assignments are passed to the macro (e.g., `is_primary_key = true`)
            #[allow(unused_mut)]
            let mut column = Column {
                name: $name,
                sql_type: $sql_type,
                is_primary_key: false,
                non_null: false,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            };
            $(
                column.$field = $value;
            )*
            column
        }
    };
}

#[derive(Debug, PartialEq, Eq)]
pub enum SqlType {
    Text,
    Integer,
    Real,
    Blob,
}

#[allow(unused)]
pub enum ForeignKeyOnChange {
    NoAction,
    Restrict,
    SetNull,
    SetDefault,
    Cascade,
}

pub struct ForeignKey {
    pub foreign_table: &'static str,
    pub foreign_column: &'static str,
    pub on_delete: ForeignKeyOnChange,
}

pub struct Column<'a, S: AsRef<str>> {
    pub name: S,
    pub sql_type: &'a SqlType,
    pub is_primary_key: bool,
    pub non_null: bool,
    pub is_unique: bool,
    pub default_value: Option<S>,
    pub foreign_key: Option<&'a ForeignKey>,
}

pub struct Table {
    pub name: &'static str,
    pub columns: &'static [Column<'static, &'static str>],
    /// Index name and column specification. The specification may end with an
    /// uppercase ` WHERE ` followed by a SQLite partial-index predicate.
    pub indices: &'static [(&'static str, &'static str)],
    pub unique_constraints: &'static [&'static [&'static str]],
}

impl Table {
    pub fn create(&self, conn: &Connection) -> Result<()> {
        super::schema_adapter::create_table(self, conn)
    }
}

pub struct VersionedSchema {
    pub version: usize,
    pub tables: &'static [Table],
    pub migration: Option<fn(&Connection) -> Result<()>>,
}

/// Check an application's ordered schema versions against its single SQLite
/// version marker. SQLite records no historical names or checksums here, so
/// this reports only the pending suffix. Callers retain their schema-shape and
/// legacy-layout checks before executing application-owned migrations.
pub fn preflight_versioned_schema(
    namespace: &str,
    schemas: &[VersionedSchema],
    current_version: Option<usize>,
) -> Result<()> {
    let plan = MigrationPlan {
        namespace: namespace.to_owned(),
        entries: schemas
            .iter()
            .map(|schema| MigrationEntry {
                version: MigrationVersion::Number(schema.version as u64),
                name: format!("schema-v{}", schema.version),
                digest: None,
            })
            .collect(),
    };
    let marker = current_version.map(|version| MigrationVersion::Number(version as u64));
    let report = plan
        .inspect_version_only(marker.as_ref())
        .map_err(|error| anyhow::anyhow!("{namespace} migration preflight failed: {error}"))?;
    tracing::debug!(
        namespace,
        applied = report.applied.len(),
        pending = report.pending.len(),
        "Migration preflight complete"
    );
    Ok(())
}

impl VersionedSchema {
    pub fn create(&self, conn: &Connection) -> Result<()> {
        conn.execute("PRAGMA foreign_keys = ON;", params![])?;
        for table in self.tables {
            table.create(conn)?;
        }
        let version = BASE_DB_VERSION + self.version;
        conn.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        Ok(())
    }
    pub fn validate(&self, conn: &Connection) -> Result<()> {
        super::schema_adapter::validate_schema(self, conn)
    }
}
pub const BASE_DB_VERSION: usize = 99999;

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PARENT_TABLE: Table = Table {
        name: "test_parent",
        columns: &[Column {
            name: "id",
            sql_type: &SqlType::Integer,
            is_primary_key: true,
            non_null: false,
            is_unique: false,
            default_value: None,
            foreign_key: None,
        }],
        indices: &[],
        unique_constraints: &[],
    };

    const TEST_PARENT_FOREIGN_KEY: ForeignKey = ForeignKey {
        foreign_table: "test_parent",
        foreign_column: "id",
        on_delete: ForeignKeyOnChange::Cascade,
    };

    const TEST_CHILD_TABLE: Table = Table {
        name: "test_child",
        columns: &[
            Column {
                name: "id",
                sql_type: &SqlType::Integer,
                is_primary_key: true,
                non_null: false,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
            Column {
                name: "parent_id",
                sql_type: &SqlType::Integer,
                is_primary_key: false,
                non_null: true,
                is_unique: false,
                default_value: None,
                foreign_key: Some(&TEST_PARENT_FOREIGN_KEY),
            },
        ],
        indices: &[],
        unique_constraints: &[],
    };

    #[test]
    fn create_enables_and_enforces_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_PARENT_TABLE, TEST_CHILD_TABLE],
            migration: None,
        };

        schema.create(&conn).unwrap();

        let foreign_keys: i64 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert!(conn
            .execute("INSERT INTO test_child (id, parent_id) VALUES (1, 999)", [],)
            .is_err());
    }

    const TEST_TABLE_WITH_INDEX: Table = Table {
        name: "test_table",
        columns: &[
            Column {
                name: "id",
                sql_type: &SqlType::Integer,
                is_primary_key: true,
                non_null: false,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
            Column {
                name: "name",
                sql_type: &SqlType::Text,
                is_primary_key: false,
                non_null: true,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
        ],
        indices: &[("idx_test_name", "name")],
        unique_constraints: &[],
    };

    #[test]
    fn test_validate_detects_missing_index() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table WITHOUT the index
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_INDEX],
            migration: None,
        };

        // Validation should fail because index is missing
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing index"));
        assert!(err_msg.contains("idx_test_name"));
    }

    #[test]
    fn test_validate_passes_with_index_present() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table WITH the index
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute("CREATE INDEX idx_test_name ON test_table(name)", [])
            .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_INDEX],
            migration: None,
        };

        // Validation should pass
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_validate_detects_index_on_wrong_table() {
        let conn = Connection::open_in_memory().unwrap();

        // Create test_table and another_table
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE another_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();

        // Create the index on the WRONG table
        conn.execute("CREATE INDEX idx_test_name ON another_table(name)", [])
            .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_INDEX],
            migration: None,
        };

        // Validation should fail because index is on wrong table
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing index"));
    }

    const TEST_TABLE_WITH_UNIQUE: Table = Table {
        name: "test_unique_table",
        columns: &[
            Column {
                name: "id",
                sql_type: &SqlType::Integer,
                is_primary_key: true,
                non_null: false,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
            Column {
                name: "email",
                sql_type: &SqlType::Text,
                is_primary_key: false,
                non_null: true,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
            Column {
                name: "username",
                sql_type: &SqlType::Text,
                is_primary_key: false,
                non_null: true,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
        ],
        indices: &[],
        unique_constraints: &[&["email", "username"]],
    };

    #[test]
    fn test_validate_detects_missing_unique_constraint() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table WITHOUT the unique constraint
        conn.execute(
            "CREATE TABLE test_unique_table (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL,
                username TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_UNIQUE],
            migration: None,
        };

        // Validation should fail because unique constraint is missing
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing unique constraint"));
        assert!(err_msg.contains("email"));
        assert!(err_msg.contains("username"));
    }

    #[test]
    fn test_validate_passes_with_unique_constraint_present() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table WITH the unique constraint
        conn.execute(
            "CREATE TABLE test_unique_table (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL,
                username TEXT NOT NULL,
                UNIQUE (email, username)
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_UNIQUE],
            migration: None,
        };

        // Validation should pass
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_validate_unique_constraint_column_order_independent() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table with unique constraint in DIFFERENT column order
        conn.execute(
            "CREATE TABLE test_unique_table (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL,
                username TEXT NOT NULL,
                UNIQUE (username, email)
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_UNIQUE],
            migration: None,
        };

        // Validation should pass (order doesn't matter for unique constraint semantics)
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_validate_detects_partial_unique_constraint() {
        let conn = Connection::open_in_memory().unwrap();

        // Create table with unique constraint on only ONE of the expected columns
        conn.execute(
            "CREATE TABLE test_unique_table (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL UNIQUE,
                username TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_UNIQUE],
            migration: None,
        };

        // Validation should fail - we have UNIQUE(email) but not UNIQUE(email, username)
        let result = schema.validate(&conn);
        assert!(result.is_err());
    }

    const PARENT_FK: ForeignKey = ForeignKey {
        foreign_table: "parent",
        foreign_column: "id",
        on_delete: ForeignKeyOnChange::Cascade,
    };

    const TEST_TABLE_WITH_FK: Table = Table {
        name: "child",
        columns: &[
            Column {
                name: "id",
                sql_type: &SqlType::Integer,
                is_primary_key: true,
                non_null: false,
                is_unique: false,
                default_value: None,
                foreign_key: None,
            },
            Column {
                name: "parent_id",
                sql_type: &SqlType::Integer,
                is_primary_key: false,
                non_null: true,
                is_unique: false,
                default_value: None,
                foreign_key: Some(&PARENT_FK),
            },
        ],
        indices: &[],
        unique_constraints: &[],
    };

    #[test]
    fn test_validate_detects_missing_foreign_key() {
        let conn = Connection::open_in_memory().unwrap();

        // Create parent table
        conn.execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        // Create child table WITHOUT foreign key
        conn.execute(
            "CREATE TABLE child (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_FK],
            migration: None,
        };

        // Validation should fail because FK is missing
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing foreign key"));
        assert!(err_msg.contains("parent_id"));
    }

    #[test]
    fn test_validate_passes_with_foreign_key_present() {
        let conn = Connection::open_in_memory().unwrap();

        // Create parent table
        conn.execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        // Create child table WITH foreign key
        conn.execute(
            "CREATE TABLE child (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE CASCADE
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_FK],
            migration: None,
        };

        // Validation should pass
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_validate_detects_wrong_on_delete_action() {
        let conn = Connection::open_in_memory().unwrap();

        // Create parent table
        conn.execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        // Create child table with FK but wrong ON DELETE action
        conn.execute(
            "CREATE TABLE child (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE SET NULL
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_FK],
            migration: None,
        };

        // Validation should fail because ON DELETE action doesn't match
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("foreign key mismatch"));
        assert!(err_msg.contains("CASCADE"));
        assert!(err_msg.contains("SET NULL"));
    }

    #[test]
    fn test_validate_detects_wrong_referenced_table() {
        let conn = Connection::open_in_memory().unwrap();

        // Create two tables
        conn.execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        conn.execute("CREATE TABLE other (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        // Create child table with FK to WRONG table
        conn.execute(
            "CREATE TABLE child (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL REFERENCES other(id) ON DELETE CASCADE
            )",
            [],
        )
        .unwrap();

        let schema = VersionedSchema {
            version: 1,
            tables: &[TEST_TABLE_WITH_FK],
            migration: None,
        };

        // Validation should fail because FK references wrong table
        let result = schema.validate(&conn);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("foreign key mismatch"));
    }
}
