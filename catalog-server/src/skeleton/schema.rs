//! Schema definition for skeleton event tables.

/// Schema definition for skeleton tables.
pub struct SkeletonSchema {
    pub version: usize,
    pub up: &'static str,
}

pub const SKELETON_VERSIONED_SCHEMAS: &[SkeletonSchema] = &[SkeletonSchema {
    version: 1,
    up: r#"
            CREATE TABLE IF NOT EXISTS catalog_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS catalog_events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                payload TEXT,
                timestamp INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_catalog_events_seq ON catalog_events(seq);
        "#,
}];
