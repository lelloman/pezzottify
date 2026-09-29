//! Driver adapter and explicit projection of Pezzottify's existing validation
//! policy. Shared code creates SQL and compares metadata; local code retains
//! descriptor syntax, one-layer default normalization and SQLite I/O.
use super::versioned_schema::{ForeignKeyOnChange, SqlType, Table, VersionedSchema};
use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use simple_server::database::sqlite::schema::*;
use std::{borrow::Cow, collections::BTreeMap};

fn id(value: impl Into<Cow<'static, str>>) -> Result<Identifier<'static>> {
    Ok(Identifier::new(value)?)
}
fn sql_type(ty: &SqlType) -> &'static str {
    match ty {
        SqlType::Text => "TEXT",
        SqlType::Integer => "INTEGER",
        SqlType::Real => "REAL",
        SqlType::Blob => "BLOB",
    }
}
fn on_delete(action: &ForeignKeyOnChange) -> ForeignKeyAction {
    match action {
        ForeignKeyOnChange::NoAction => ForeignKeyAction::NoAction,
        ForeignKeyOnChange::Restrict => ForeignKeyAction::Restrict,
        ForeignKeyOnChange::SetNull => ForeignKeyAction::SetNull,
        ForeignKeyOnChange::SetDefault => ForeignKeyAction::SetDefault,
        ForeignKeyOnChange::Cascade => ForeignKeyAction::Cascade,
    }
}
fn action(value: &str) -> Result<ForeignKeyAction> {
    Ok(match value {
        "NO ACTION" => ForeignKeyAction::NoAction,
        "RESTRICT" => ForeignKeyAction::Restrict,
        "SET NULL" => ForeignKeyAction::SetNull,
        "SET DEFAULT" => ForeignKeyAction::SetDefault,
        "CASCADE" => ForeignKeyAction::Cascade,
        _ => bail!("Unknown SQLite foreign key action: {value}"),
    })
}
// This is intentionally the existing application rule, not general SQL
// equivalence. Keep it at the adapter boundary and apply once to both sides.
fn normalize_default(s: &str) -> &str {
    if s.starts_with('(') && s.ends_with(')') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}
fn default_value(s: &str, validation: bool) -> Result<SqlExpression<'static>> {
    Ok(SqlExpression::trusted(
        if validation { normalize_default(s) } else { s }.to_owned(),
    )?)
}

fn table_spec(table: &Table, validation: bool) -> Result<TableSpec<'static>> {
    let columns = table
        .columns
        .iter()
        .map(|c| {
            Ok(ColumnSpec {
                name: id(c.name)?,
                declared_type: sql_type(c.sql_type).into(),
                not_null: c.non_null,
                default: c
                    .default_value
                    .map(|s| default_value(s, validation))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let primary_key = table
        .columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| id(c.name))
        .collect::<Result<Vec<_>>>()?;
    // The local descriptor has single-column PRIMARY KEY flags; don't silently
    // reinterpret multiple flags as a composite key during creation.
    if primary_key.len() > 1 {
        bail!("Table {} declares multiple primary keys", table.name);
    }
    let mut unique_constraints = table
        .unique_constraints
        .iter()
        .map(|cols| cols.iter().map(|c| id(*c)).collect())
        .collect::<Result<Vec<Vec<_>>>>()?;
    // Existing validation deliberately does not check column is_unique flags.
    if !validation {
        for c in table.columns.iter().filter(|c| c.is_unique) {
            let key = vec![id(c.name)?];
            if !unique_constraints.contains(&key) {
                unique_constraints.push(key);
            }
        }
    }
    let foreign_keys = table
        .columns
        .iter()
        .filter_map(|c| c.foreign_key.map(|fk| (c, fk)))
        .map(|(c, fk)| {
            Ok(ForeignKeySpec {
                columns: vec![id(c.name)?].into(),
                parent_table: id(fk.foreign_table)?,
                parent_columns: vec![id(fk.foreign_column)?].into(),
                on_update: ForeignKeyAction::NoAction,
                on_delete: on_delete(&fk.on_delete),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let indexes = table
        .indices
        .iter()
        .map(|(name, spec)| {
            let (columns, predicate) = spec
                .split_once(" WHERE ")
                .map_or((*spec, None), |(a, b)| (a, Some(b)));
            let terms = columns
                .split(',')
                .map(|term| {
                    let words = term.split_whitespace().collect::<Vec<_>>();
                    let (column, descending) = match words.as_slice() {
                        [column] => (*column, false),
                        [column, "ASC"] => (*column, false),
                        [column, "DESC"] => (*column, true),
                        _ => bail!("Unsupported application index term {term:?}"),
                    };
                    Ok(IndexTerm {
                        column: id(column.to_owned())?,
                        collation: id("BINARY")?,
                        descending,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(IndexSpec {
                name: id(*name)?,
                terms: terms.into(),
                unique: false,
                predicate: predicate
                    .map(|s| SqlExpression::trusted(s.to_owned()))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(TableSpec {
        name: id(table.name)?,
        columns: columns.into(),
        primary_key: primary_key.into(),
        unique_constraints: unique_constraints.into(),
        foreign_keys: foreign_keys.into(),
        indexes: indexes.into(),
        unsupported: Cow::Borrowed(&[]),
    })
}
fn snapshot(version: usize, tables: Vec<TableSpec<'static>>) -> Result<SchemaSnapshot<'static>> {
    Ok(SchemaSnapshot {
        namespace: Cow::Borrowed("pezzottify/versioned-schema"),
        database: id("main")?,
        version: version as u64,
        tables: tables.into(),
    })
}

pub(super) fn create_table(table: &Table, conn: &Connection) -> Result<()> {
    let schema = snapshot(0, vec![table_spec(table, false)?])?;
    for command in create_plan(&schema)?.statements {
        conn.execute(&command, [])
            .with_context(|| format!("Creating table/index for {}", table.name))?;
    }
    Ok(())
}

fn profile() -> ValidationProfile {
    ValidationProfile {
        policy: ValidationPolicy {
            scope: ValidationScope::RequiredSubset,
            column_order: ColumnOrder::Ordered,
            declared_types: TypeComparison::Exact,
            expressions: ExpressionComparison::Exact,
        },
        columns: ValidationScope::Exact,
        column_names: NameComparison::Exact,
        indexes: IndexComparison::NamesOnly,
        unique_constraints: UniqueComparison::UnorderedColumnSets,
        foreign_keys: ForeignKeyComparison::DeleteActionPerColumn,
        primary_key: PrimaryKeyComparison::FirstColumn,
        verify_table_properties: false,
    }
}
fn unavailable<T>() -> Observed<T> {
    Observed::Unavailable("Outside Pezzottify's selected metadata projection".into())
}

fn observe_table(
    conn: &Connection,
    name: String,
    selected: bool,
) -> Result<TableObservation<'static>> {
    let mut table = TableObservation {
        name: id(name.clone())?,
        columns: unavailable(),
        primary_key: unavailable(),
        unique_constraints: unavailable(),
        foreign_keys: unavailable(),
        indexes: unavailable(),
        index_names: unavailable(),
        unique_column_sets: unavailable(),
        ordinary_table: unavailable(),
    };
    if !selected {
        return Ok(table);
    }
    // Preserve the historical table_info projection. Hidden/generated columns,
    // CHECKs, STRICT/WITHOUT ROWID and other DDL properties are explicitly outside
    // this profile; we do not assert ordinary_table = Known(()).
    let mut stmt = conn.prepare(
        "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1) ORDER BY cid",
    )?;
    let rows = stmt
        .query_map([&name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut pk = vec![];
    let mut columns = vec![];
    for (name, ty, not_null, default, position) in rows {
        if position > 0 {
            pk.push((position, id(name.clone())?));
        }
        columns.push(ColumnSpec {
            name: id(name)?,
            declared_type: ty.into(),
            not_null,
            default: default.map(|s| default_value(&s, true)).transpose()?,
        });
    }
    pk.sort_by_key(|(n, _)| *n);
    table.columns = Observed::Known(columns);
    table.primary_key = Observed::Known(pk.into_iter().map(|(_, name)| name).collect());
    let mut stmt = conn.prepare("SELECT name, \"unique\" FROM pragma_index_list(?1)")?;
    let indexes = stmt
        .query_map([&name], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    table.index_names = Observed::Known(
        indexes
            .iter()
            .map(|(name, _)| id(name.clone()))
            .collect::<Result<_>>()?,
    );
    let mut unique_sets = vec![];
    for (name, unique) in indexes {
        if unique {
            let mut stmt = conn.prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")?;
            let names = stmt
                .query_map([&name], |r| r.get::<_, Option<String>>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // A named-column projection, explicitly excluding expression terms;
            // decoding errors propagate instead of being silently discarded.
            unique_sets.push(
                names
                    .into_iter()
                    .flatten()
                    .map(id)
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    table.unique_column_sets = Observed::Known(unique_sets);
    let mut stmt = conn.prepare("SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete FROM pragma_foreign_key_list(?1) ORDER BY id, seq")?;
    let rows = stmt
        .query_map([&name], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut fks: BTreeMap<i64, ForeignKeySpec<'static>> = BTreeMap::new();
    for (n, parent, from, to, update, delete) in rows {
        let on_update = action(&update)?;
        let on_delete = action(&delete)?;
        let fk = fks.entry(n).or_insert(ForeignKeySpec {
            columns: vec![].into(),
            parent_table: id(parent)?,
            parent_columns: vec![].into(),
            on_update,
            on_delete,
        });
        fk.columns.to_mut().push(id(from)?);
        fk.parent_columns.to_mut().push(id(to)?);
    }
    table.foreign_keys = Observed::Known(fks.into_values().collect());
    Ok(table)
}

pub(super) fn validation_report(
    schema: &VersionedSchema,
    conn: &Connection,
) -> Result<SchemaReport> {
    let snapshot = snapshot(
        schema.version,
        schema
            .tables
            .iter()
            .map(|t| table_spec(t, true))
            .collect::<Result<_>>()?,
    )?;
    let mut stmt = conn.prepare("SELECT type, name FROM sqlite_master WHERE name NOT GLOB 'sqlite_*' AND type IN ('table','view','trigger') ORDER BY name")?;
    let inventory = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut tables = vec![];
    let mut other_objects = vec![];
    for (kind, name) in inventory {
        if kind == "table" {
            let selected = schema
                .tables
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(&name));
            tables.push(observe_table(conn, name, selected)?);
        } else {
            other_objects.push(SchemaObject {
                kind: kind.into(),
                name: id(name)?,
            });
        }
    }
    let observed = SchemaObservation {
        namespace: snapshot.namespace.clone(),
        database: id("main")?,
        tables: Observed::Known(tables),
        other_objects: Observed::Known(other_objects),
    };
    Ok(validate_with_profile(&snapshot, &observed, profile())?)
}

pub(super) fn validate_schema(schema: &VersionedSchema, conn: &Connection) -> Result<()> {
    let report = validation_report(schema, conn)?;
    tracing::debug!(version = schema.version, checked = report.checked.len(), excluded = ?report.outside_scope, "Shared SQLite schema projection checked");
    if let Some(diff) = report.differences.first() {
        let reason = if diff.path.ends_with("index_names") {
            "missing index"
        } else if diff.path.ends_with("unique_column_sets") {
            "missing unique constraint"
        } else if diff.path.ends_with("foreign_keys") {
            "missing foreign key or foreign key mismatch"
        } else {
            "schema mismatch"
        };
        // Keep readable startup errors and the established constraint keywords.
        // All individual shared differences remain visible in the diagnostic.
        bail!(
            "{reason} at {}: expected {:?}, observed {:?}; differences: {:?}",
            diff.path,
            diff.expected,
            diff.observed,
            report.differences
        );
    }
    Ok(())
}
