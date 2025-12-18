use anyhow::{bail, Result};
use rusqlite::{params, types::Type, Connection};

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
    pub indices: &'static [(&'static str, &'static str)],
    pub unique_constraints: &'static [&'static [&'static str]],
}

impl Table {
    pub fn create(&self, conn: &Connection) -> Result<()> {
        let mut create_sql = format!("CREATE TABLE {} (", self.name);
        for (column_index, column) in self.columns.iter().enumerate() {
            if column_index > 0 {
                create_sql.push_str(", ");
            }
            create_sql.push_str(&format!(
                "{} {}",
                column.name,
                match column.sql_type {
                    SqlType::Text => "TEXT",
                    SqlType::Integer => "INTEGER",
                    SqlType::Real => "REAL",
                    SqlType::Blob => "BLOB",
                }
            ));
            if column.is_primary_key {
                create_sql.push_str(" PRIMARY KEY");
            }
            if column.non_null {
                create_sql.push_str(" NOT NULL");
            }
            if column.is_unique {
                create_sql.push_str(" UNIQUE");
            }
            if let Some(default_value) = column.default_value {
                create_sql.push_str(&format!(" DEFAULT {}", default_value));
            }
            if let Some(foreign_key) = column.foreign_key {
                create_sql.push_str(&format!(
                    " REFERENCES {}({}) ON DELETE {}",
                    foreign_key.foreign_table,
                    foreign_key.foreign_column,
                    match foreign_key.on_delete {
                        ForeignKeyOnChange::NoAction => "NO ACTION",
                        ForeignKeyOnChange::Restrict => "RESTRICT",
                        ForeignKeyOnChange::SetNull => "SET NULL",
                        ForeignKeyOnChange::SetDefault => "SET DEFAULT",
                        ForeignKeyOnChange::Cascade => "CASCADE",
                    }
                ));
            }
        }

        for unique_constraint in self.unique_constraints {
            create_sql.push_str(&format!(", UNIQUE ({})", unique_constraint.join(", ")));
        }
        create_sql.push_str(");");
        conn.execute(&create_sql, params![])?;

        for (index_name, column_name) in self.indices {
            conn.execute(
                &format!(
                    "CREATE INDEX {} ON {}({});",
                    index_name, self.name, column_name
                ),
                params![],
            )?;
        }
        Ok(())
    }
}

pub struct VersionedSchema {
    pub version: usize,
    pub tables: &'static [Table],
    pub migration: Option<fn(&Connection) -> Result<()>>,
}

fn strip_leading_and_trailing_parentheses<S: AsRef<str>>(s: S) -> String {
    let s = s.as_ref();
    if s.starts_with('(') && s.ends_with(')') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

impl VersionedSchema {
    pub fn create(&self, conn: &Connection) -> Result<()> {
        conn.execute("PRAGMA foreign_keys = ON;", params![])?;
        for table in self.tables {
            table.create(conn)?;
        }
        conn.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + self.version),
            [],
        )?;
        Ok(())
    }
    pub fn validate(&self, conn: &Connection) -> Result<()> {
        for table in self.tables {
            let mut stmt = conn.prepare(&format!("PRAGMA table_info({});", table.name))?;
            let actual_columns: Vec<Result<Column<'_, String>, rusqlite::Error>> = stmt
                .query_map(params![], |row| {
                    let name = row.get::<usize, String>(1)?;
                    let sql_type = match row.get::<_, String>(2)?.as_str() {
                        "TEXT" => &SqlType::Text,
                        "INTEGER" => &SqlType::Integer,
                        "REAL" => &SqlType::Real,
                        "BLOB" => &SqlType::Blob,
                        _ => {
                            return Err(rusqlite::Error::InvalidColumnType(
                                2,
                                "".to_string(),
                                Type::Text,
                            ))
                        }
                    };

                    Ok(Column {
                        name,
                        sql_type,
                        non_null: row.get::<_, i32>(3)? == 1,
                        default_value: row
                            .get::<_, Option<String>>(4)?
                            .as_deref()
                            .map(|s| s.to_string()),
                        is_primary_key: row.get::<_, i32>(5)? == 1,
                        is_unique: false,
                        foreign_key: None,
                    })
                })?
                .collect();

            if actual_columns.len() != table.columns.len() {
                bail!(
                    "Table {} has {} columns, expected {}. Found column names: {}, expected: {}",
                    table.name,
                    actual_columns.len(),
                    table.columns.len(),
                    actual_columns
                        .iter()
                        .filter_map(|c| {
                            if let Ok(column) = c {
                                Some(column.name.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                    table
                        .columns
                        .iter()
                        .map(|c| c.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            for (actual_column_result, expected_column) in
                actual_columns.iter().zip(table.columns.iter())
            {
                let actual_column = match actual_column_result {
                    Ok(column) => column,
                    Err(e) => bail!("Error reading column: {:?}", e),
                };
                if actual_column.name != expected_column.name {
                    bail!(
                        "Table {} Column name mismatch: expected {}, got {}",
                        &table.name,
                        expected_column.name,
                        actual_column.name
                    );
                }
                if actual_column.sql_type != expected_column.sql_type {
                    bail!(
                        "Table {} Column {} type mismatch: expected {:?}, got {:?}",
                        &table.name,
                        expected_column.name,
                        expected_column.sql_type,
                        actual_column.sql_type
                    );
                }
                if actual_column.non_null != expected_column.non_null {
                    bail!(
                        "Table {} Column {} non-null mismatch: expected {}, got {}",
                        &table.name,
                        expected_column.name,
                        expected_column.non_null,
                        actual_column.non_null
                    );
                }

                // Default values might be wrapped in parentheses, so we strip them before comparing
                if actual_column
                    .default_value
                    .as_ref()
                    .map(strip_leading_and_trailing_parentheses)
                    != expected_column
                        .default_value
                        .map(strip_leading_and_trailing_parentheses)
                {
                    bail!(
                        "Table {} Column {} default value mismatch: expected {:?}, got {:?}",
                        &table.name,
                        expected_column.name,
                        expected_column.default_value,
                        actual_column.default_value
                    );
                }
                if actual_column.is_primary_key != expected_column.is_primary_key {
                    bail!(
                        "Table {} Column {} primary key mismatch: expected {}, got {}",
                        &table.name,
                        expected_column.name,
                        expected_column.is_primary_key,
                        actual_column.is_primary_key
                    );
                }
            }

            // Validate indices exist
            for (index_name, _columns) in table.indices {
                let index_exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1 AND tbl_name=?2",
                        params![index_name, table.name],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !index_exists {
                    bail!(
                        "Table {} is missing index '{}'",
                        table.name,
                        index_name
                    );
                }
            }

            // Validate unique constraints exist
            // SQLite stores unique constraints as indices with unique=1 in PRAGMA index_list
            if !table.unique_constraints.is_empty() {
                // Get all unique indices for this table (query once, use for all constraints)
                let mut stmt = conn.prepare(&format!(
                    "PRAGMA index_list({})",
                    table.name
                ))?;
                let unique_indices: Vec<String> = stmt
                    .query_map([], |row| {
                        let name: String = row.get(1)?;
                        let is_unique: i32 = row.get(2)?;
                        Ok((name, is_unique))
                    })?
                    .filter_map(|r| r.ok())
                    .filter(|(_, is_unique)| *is_unique == 1)
                    .map(|(name, _)| name)
                    .collect();

                // Build a list of all unique index column sets for comparison
                let mut unique_index_columns: Vec<Vec<String>> = Vec::new();
                for index_name in &unique_indices {
                    let mut idx_stmt = conn.prepare(&format!(
                        "PRAGMA index_info({})",
                        index_name
                    ))?;
                    let mut cols: Vec<String> = idx_stmt
                        .query_map([], |row| row.get::<_, String>(2))?
                        .filter_map(|r| r.ok())
                        .collect();
                    cols.sort();
                    unique_index_columns.push(cols);
                }

                for expected_columns in table.unique_constraints {
                    let expected_cols_sorted: Vec<&str> = {
                        let mut cols: Vec<&str> = expected_columns.iter().copied().collect();
                        cols.sort();
                        cols
                    };

                    let found = unique_index_columns.iter().any(|actual_cols| {
                        actual_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>() == expected_cols_sorted
                    });

                    if !found {
                        bail!(
                            "Table {} is missing unique constraint on columns ({})",
                            table.name,
                            expected_columns.join(", ")
                        );
                    }
                }
            }

            // Validate foreign keys exist and match expected configuration
            // PRAGMA foreign_key_list returns: id, seq, table, from, to, on_update, on_delete, match
            let mut fk_stmt = conn.prepare(&format!(
                "PRAGMA foreign_key_list({})",
                table.name
            ))?;

            struct ActualFk {
                from_column: String,
                to_table: String,
                to_column: String,
                on_delete: String,
            }

            let actual_fks: Vec<ActualFk> = fk_stmt
                .query_map([], |row| {
                    Ok(ActualFk {
                        from_column: row.get(3)?,
                        to_table: row.get(2)?,
                        to_column: row.get(4)?,
                        on_delete: row.get(6)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            for column in table.columns {
                if let Some(expected_fk) = column.foreign_key {
                    let expected_on_delete = match expected_fk.on_delete {
                        ForeignKeyOnChange::NoAction => "NO ACTION",
                        ForeignKeyOnChange::Restrict => "RESTRICT",
                        ForeignKeyOnChange::SetNull => "SET NULL",
                        ForeignKeyOnChange::SetDefault => "SET DEFAULT",
                        ForeignKeyOnChange::Cascade => "CASCADE",
                    };

                    let found = actual_fks.iter().any(|actual| {
                        actual.from_column == column.name
                            && actual.to_table == expected_fk.foreign_table
                            && actual.to_column == expected_fk.foreign_column
                            && actual.on_delete == expected_on_delete
                    });

                    if !found {
                        // Check if FK exists but with wrong configuration
                        let partial_match = actual_fks.iter().find(|actual| {
                            actual.from_column == column.name
                        });

                        if let Some(actual) = partial_match {
                            bail!(
                                "Table {} column {} has foreign key mismatch: expected REFERENCES {}({}) ON DELETE {}, got REFERENCES {}({}) ON DELETE {}",
                                table.name,
                                column.name,
                                expected_fk.foreign_table,
                                expected_fk.foreign_column,
                                expected_on_delete,
                                actual.to_table,
                                actual.to_column,
                                actual.on_delete
                            );
                        } else {
                            bail!(
                                "Table {} column {} is missing foreign key: expected REFERENCES {}({}) ON DELETE {}",
                                table.name,
                                column.name,
                                expected_fk.foreign_table,
                                expected_fk.foreign_column,
                                expected_on_delete
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
pub const BASE_DB_VERSION: usize = 99999;

#[cfg(test)]
mod tests {
    use super::*;

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
