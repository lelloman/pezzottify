use anyhow::{bail, Result};
use rusqlite::{params, types::Type, Connection};

pub const DEFAULT_TIMESTAMP: &'static str = "(cast(strftime('%s','now') as int))";

#[macro_export]
macro_rules! sqlite_column {
    ($name:expr, $sql_type:expr $(, $field:ident = $value:expr)*) => {
        {
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
    pub indices: &'static [&'static str],
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

        for index in self.indices {
            conn.execute(
                &format!("CREATE INDEX {} ON {}({});", index, self.name, index),
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
                        name: name,
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
                    "Table {} has {} columns, expected {}",
                    table.name,
                    actual_columns.len(),
                    table.columns.len()
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
        }
        Ok(())
    }
}
pub const BASE_DB_VERSION: usize = 99999;
