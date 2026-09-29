use super::{legacy_schema_test_oracle as legacy, schema_adapter, *};
use crate::{
    backup::DbRegistry,
    server_store::{ServerStore, SqliteServerStore, SERVER_VERSIONED_SCHEMAS},
};
use rusqlite::Connection;

fn legacy_create(schema: &VersionedSchema, conn: &Connection) {
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    for table in schema.tables {
        legacy::create_table(table, conn).unwrap();
    }
    conn.pragma_update(None, "user_version", BASE_DB_VERSION + schema.version)
        .unwrap();
}
fn history_parity(history: &[VersionedSchema]) {
    for schema in history {
        let old = Connection::open_in_memory().unwrap();
        legacy_create(schema, &old);
        let new = Connection::open_in_memory().unwrap();
        schema.create(&new).unwrap();
        // Validation intentionally checks index presence only. Independently
        // compare actual generated index behavior so the creator cannot hide a
        // wrong column order, collation, direction or partial predicate.
        for table in schema.tables {
            for (name, _) in table.indices {
                let terms = |conn: &Connection| {
                    conn.prepare("SELECT seqno, cid, name, desc, coll, key FROM pragma_index_xinfo(?1) ORDER BY seqno").unwrap()
                        .query_map([name], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, bool>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, bool>(5)?))).unwrap()
                        .collect::<rusqlite::Result<Vec<_>>>().unwrap()
                };
                assert_eq!(terms(&old), terms(&new), "v{} index {name}", schema.version);
                let predicate = |conn: &Connection| {
                    let ddl: String = conn
                        .query_row(
                            "SELECT sql FROM sqlite_master WHERE type='index' AND name=?1",
                            [name],
                            |r| r.get(0),
                        )
                        .unwrap();
                    ddl.split_once(" WHERE ").map(|(_, p)| p.to_owned())
                };
                assert_eq!(
                    predicate(&old),
                    predicate(&new),
                    "v{} index {name}",
                    schema.version
                );
            }
        }
        for (source, conn) in [("legacy creation", &old), ("shared creation", &new)] {
            legacy::validate_schema(schema, conn).unwrap_or_else(|e| {
                panic!("{source}, v{}: legacy validation: {e:#}", schema.version)
            });
            schema.validate(conn).unwrap_or_else(|e| {
                panic!("{source}, v{}: shared validation: {e:#}", schema.version)
            });
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |r| r.get::<_, usize>(0))
                    .unwrap(),
                BASE_DB_VERSION + schema.version
            );
            let report = schema_adapter::validation_report(schema, conn).unwrap();
            assert!(report.is_match());
            assert!(schema.tables.is_empty() || !report.outside_scope.is_empty());
        }
    }
}
#[test]
fn all_user_versions_accept_legacy_and_shared_creation() {
    history_parity(crate::user::USER_SCHEMA_CANARY_HISTORY);
}
#[test]
fn all_server_versions_accept_legacy_and_shared_creation() {
    history_parity(SERVER_VERSIONED_SCHEMAS);
}
#[test]
fn all_catalog_versions_accept_legacy_and_shared_creation() {
    history_parity(crate::catalog_store::CATALOG_VERSIONED_SCHEMAS);
}
#[test]
fn all_enrichment_versions_accept_legacy_and_shared_creation() {
    history_parity(crate::enrichment_store::ENRICHMENT_SCHEMA_CANARY_HISTORY);
}
#[test]
fn all_download_versions_accept_legacy_and_shared_creation() {
    history_parity(crate::download_manager::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS);
}

const PARENT: Table = Table {
    name: "parent",
    columns: &[crate::sqlite_column!(
        "id",
        &SqlType::Integer,
        is_primary_key = true
    )],
    indices: &[],
    unique_constraints: &[],
};
const FK: ForeignKey = ForeignKey {
    foreign_table: "parent",
    foreign_column: "id",
    on_delete: ForeignKeyOnChange::Cascade,
};
const CHILD: Table = Table {
    name: "child",
    columns: &[
        crate::sqlite_column!("id", &SqlType::Integer, is_primary_key = true),
        crate::sqlite_column!("parent_id", &SqlType::Integer, foreign_key = Some(&FK)),
        crate::sqlite_column!(
            "a",
            &SqlType::Text,
            non_null = true,
            default_value = Some("'a'")
        ),
        crate::sqlite_column!("b", &SqlType::Text),
    ],
    indices: &[("idx_child", "a")],
    unique_constraints: &[&["a", "b"]],
};
const SHAPE: VersionedSchema = VersionedSchema {
    version: 1,
    tables: &[PARENT, CHILD],
    migration: None,
};

fn shape(ddl: &str) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY);")
        .unwrap();
    conn.execute_batch(ddl).unwrap();
    conn
}
fn assert_parity(conn: &Connection, expected: bool) {
    let old = legacy::validate_schema(&SHAPE, conn);
    let new = SHAPE.validate(conn);
    assert_eq!(old.is_ok(), expected, "oracle: {old:?}");
    assert_eq!(new.is_ok(), expected, "shared: {new:?}");
}
#[test]
fn mixed_policy_preserves_historical_acceptance_without_claiming_more() {
    let conn = shape("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE ON UPDATE RESTRICT, a TEXT NOT NULL DEFAULT ('a'), b TEXT, UNIQUE(b,a)); CREATE INDEX idx_child ON child(b DESC) WHERE b IS NOT NULL; CREATE VIEW extra AS SELECT * FROM child; CREATE TABLE auxiliary(x);");
    assert_parity(&conn, true);
    let report = schema_adapter::validation_report(&SHAPE, &conn).unwrap();
    assert!(report
        .outside_scope
        .iter()
        .any(|s| s.ends_with("ordinary_table")));
    assert!(report
        .outside_scope
        .iter()
        .any(|s| s.contains("terms_uniqueness_predicates")));
    assert!(report
        .outside_scope
        .iter()
        .any(|s| s.contains("update_action_grouping")));
}
#[test]
fn all_existing_rejection_boundaries_remain_rejections() {
    let base = "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE, a TEXT NOT NULL DEFAULT 'a', b TEXT, UNIQUE(a,b)); CREATE INDEX idx_child ON child(a);";
    for ddl in [
        base.replace("b TEXT,", "b INTEGER,"),
        base.replace("a TEXT NOT NULL", "a TEXT"),
        base.replace("DEFAULT 'a'", "DEFAULT 'different'"),
        base.replace("b TEXT,", "b TEXT, unexpected TEXT,"),
        base.replace("b TEXT,", "B TEXT,"),
        base.replace("UNIQUE(a,b)", "UNIQUE(a)"),
        base.replace(" ON DELETE CASCADE", " ON DELETE SET NULL"),
        base.replace(" REFERENCES parent(id) ON DELETE CASCADE", ""),
        base.replace("CREATE INDEX idx_child ON child(a);", ""),
        base.replace(
            "CREATE INDEX idx_child ON child(a);",
            "CREATE INDEX idx_child ON parent(id);",
        ),
        base.replace("INTEGER PRIMARY KEY", "INTEGER"),
    ] {
        assert_parity(&shape(&ddl), false);
    }
    assert_parity(&shape(base), true);
}
#[test]
fn expression_unique_projection_and_unmodeled_checks_keep_legacy_scope() {
    let conn = shape("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE, a TEXT NOT NULL DEFAULT 'a', b TEXT CHECK(length(b)>0)); CREATE UNIQUE INDEX candidate ON child(b,a,length(b)) WHERE b IS NOT NULL; CREATE INDEX idx_child ON child(b);");
    assert_parity(&conn, true);
    assert!(schema_adapter::validation_report(&SHAPE, &conn)
        .unwrap()
        .outside_scope
        .iter()
        .any(|s| s.contains("ordering_expressions_predicates_collations")));
}
#[test]
fn selected_metadata_read_failures_are_returned_as_errors() {
    use rusqlite::hooks::{AuthAction, Authorization};
    let conn = shape("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE, a TEXT NOT NULL DEFAULT 'a', b TEXT, UNIQUE(a,b)); CREATE INDEX idx_child ON child(a);");
    conn.authorizer(Some(|context: rusqlite::hooks::AuthContext<'_>| {
        if matches!(
            context.action,
            AuthAction::Read {
                table_name: "sqlite_master",
                ..
            }
        ) {
            Authorization::Deny
        } else {
            Authorization::Allow
        }
    }));
    let error = SHAPE.validate(&conn).unwrap_err();
    assert!(
        error.downcast_ref::<rusqlite::Error>().is_some(),
        "{error:#}"
    );
}

fn historical_server(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    legacy_create(&SERVER_VERSIONED_SCHEMAS[0], &conn);
    conn.execute("INSERT INTO job_runs(job_id,started_at,status,triggered_by) VALUES ('retained','2026-01-01T00:00:00Z','completed','test')", []).unwrap();
}
#[test]
fn production_server_opens_upgrades_and_restarts_legacy_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.db");
    historical_server(&path);
    let registry = DbRegistry::new();
    let store = SqliteServerStore::new(&path, &registry).unwrap();
    assert_eq!(registry.all(), vec![path.clone()]);
    assert_eq!(store.get_job_history("retained", 5).unwrap().len(), 1);
    store.set_state("canary", "preserved").unwrap();
    drop(store);
    let reopened = SqliteServerStore::new(&path, &DbRegistry::new()).unwrap();
    assert_eq!(
        reopened.get_state("canary").unwrap().as_deref(),
        Some("preserved")
    );
    assert_eq!(reopened.get_job_history("retained", 5).unwrap().len(), 1);
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        BASE_DB_VERSION + SERVER_VERSIONED_SCHEMAS.last().unwrap().version
    );
}
#[test]
fn failed_production_upgrade_rolls_back_and_can_retry_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.db");
    historical_server(&path);
    // The v4 collision occurs after earlier migrations have already run in the
    // same transaction. Failure must preserve rows/marker and roll those back.
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE bug_reports(blocker TEXT)")
        .unwrap();
    let registry = DbRegistry::new();
    assert!(SqliteServerStore::new(&path, &registry).is_err());
    assert!(registry.all().is_empty());
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        BASE_DB_VERSION + 1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='server_state'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT job_id FROM job_runs", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "retained"
    );
    conn.execute_batch("DROP TABLE bug_reports").unwrap();
    drop(conn);
    assert!(SqliteServerStore::new(&path, &DbRegistry::new()).is_ok());
}
#[test]
fn production_schema_drift_fails_before_upgrade_or_backup_registration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.db");
    historical_server(&path);
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP INDEX idx_job_runs_status")
        .unwrap();
    let registry = DbRegistry::new();
    let error = match SqliteServerStore::new(&path, &registry) {
        Ok(_) => panic!("unexpected startup success"),
        Err(e) => e,
    };
    assert!(format!("{error:#}").contains("missing index"));
    assert!(registry.all().is_empty());
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        BASE_DB_VERSION + 1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='server_state'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
