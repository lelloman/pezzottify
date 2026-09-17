-- Additive MusicBrainz Work graph storage. External IDs, not names, identify nodes.
CREATE TABLE IF NOT EXISTS work_source_evidence_v1 (
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    work_id TEXT NOT NULL REFERENCES works_v1(id),
    source_snapshot TEXT NOT NULL,
    evidence_json TEXT NOT NULL,
    imported_at INTEGER NOT NULL,
    PRIMARY KEY(provider, external_id)
);
CREATE INDEX IF NOT EXISTS work_source_evidence_work ON work_source_evidence_v1(work_id);
CREATE TABLE IF NOT EXISTS work_relationships_v1 (
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    source_work_id TEXT NOT NULL REFERENCES works_v1(id),
    target_work_id TEXT NOT NULL REFERENCES works_v1(id),
    relationship_type TEXT NOT NULL,
    relationship_type_id TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    evidence_json TEXT NOT NULL,
    source_snapshot TEXT NOT NULL,
    imported_at INTEGER NOT NULL,
    PRIMARY KEY(provider, external_id)
);
CREATE INDEX IF NOT EXISTS work_relationships_source ON work_relationships_v1(source_work_id, relationship_type, ordering);
CREATE INDEX IF NOT EXISTS work_relationships_target ON work_relationships_v1(target_work_id, relationship_type);
CREATE TABLE IF NOT EXISTS work_import_batches_v1 (
    id TEXT PRIMARY KEY,
    imported_at INTEGER NOT NULL,
    manifest_json TEXT NOT NULL,
    report_json TEXT NOT NULL
);
