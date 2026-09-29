//! Frozen pre-07e behavior; test-only compatibility oracle, not production code.
use super::versioned_schema::*;
use anyhow::{bail, Result};
use rusqlite::{params, types::Type, Connection};
pub(super) fn create_table(table: &Table, conn: &Connection) -> Result<()> {
    let mut create_sql = format!("CREATE TABLE {} (", table.name);
    for (column_index, column) in table.columns.iter().enumerate() {
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

    for unique_constraint in table.unique_constraints {
        create_sql.push_str(&format!(", UNIQUE ({})", unique_constraint.join(", ")));
    }
    create_sql.push_str(");");
    conn.execute(&create_sql, params![])?;

    for (index_name, index_spec) in table.indices {
        let (columns, predicate) = match index_spec.split_once(" WHERE ") {
            Some((columns, predicate)) => (columns, Some(predicate)),
            None => (*index_spec, None),
        };
        let where_clause = predicate
            .map(|predicate| format!(" WHERE {predicate}"))
            .unwrap_or_default();
        conn.execute(
            &format!(
                "CREATE INDEX {} ON {}({}){};",
                index_name, table.name, columns, where_clause
            ),
            params![],
        )?;
    }
    Ok(())
}
fn strip_leading_and_trailing_parentheses<S: AsRef<str>>(s: S) -> String {
    let s = s.as_ref();
    if s.starts_with('(') && s.ends_with(')') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
pub(super) fn validate_schema(schema: &VersionedSchema, conn: &Connection) -> Result<()> {
    for table in schema.tables {
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
                    table.name,
                    expected_column.name,
                    actual_column.name
                );
            }
            if actual_column.sql_type != expected_column.sql_type {
                bail!(
                    "Table {} Column {} type mismatch: expected {:?}, got {:?}",
                    table.name,
                    expected_column.name,
                    expected_column.sql_type,
                    actual_column.sql_type
                );
            }
            if actual_column.non_null != expected_column.non_null {
                bail!(
                    "Table {} Column {} non-null mismatch: expected {}, got {}",
                    table.name,
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
                    table.name,
                    expected_column.name,
                    expected_column.default_value,
                    actual_column.default_value
                );
            }
            if actual_column.is_primary_key != expected_column.is_primary_key {
                bail!(
                    "Table {} Column {} primary key mismatch: expected {}, got {}",
                    table.name,
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
                bail!("Table {} is missing index '{}'", table.name, index_name);
            }
        }

        // Validate unique constraints exist
        // SQLite stores unique constraints as indices with unique=1 in PRAGMA index_list
        if !table.unique_constraints.is_empty() {
            // Get all unique indices for this table (query once, use for all constraints)
            let mut stmt = conn.prepare(&format!("PRAGMA index_list({})", table.name))?;
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
                let mut idx_stmt = conn.prepare(&format!("PRAGMA index_info({})", index_name))?;
                let mut cols: Vec<String> = idx_stmt
                    .query_map([], |row| row.get::<_, String>(2))?
                    .filter_map(|r| r.ok())
                    .collect();
                cols.sort();
                unique_index_columns.push(cols);
            }

            for expected_columns in table.unique_constraints {
                let expected_cols_sorted: Vec<&str> = {
                    let mut cols: Vec<&str> = expected_columns.to_vec();
                    cols.sort();
                    cols
                };

                let found = unique_index_columns.iter().any(|actual_cols| {
                    actual_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        == expected_cols_sorted
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
        let mut fk_stmt = conn.prepare(&format!("PRAGMA foreign_key_list({})", table.name))?;

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
                    let partial_match = actual_fks
                        .iter()
                        .find(|actual| actual.from_column == column.name);

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
