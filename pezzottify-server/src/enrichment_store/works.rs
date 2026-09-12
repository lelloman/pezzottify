//! Work identity is separate from a recording's title, performer and ISRC.
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteEnrichmentStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Work {
    pub id: String,
    pub title: String,
    pub creators: Vec<String>,
    pub catalog_number: Option<String>,
    pub kind: String,
    pub created_at: i64,
}

/// A model proposes identity; storage decides whether it can be accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkProposal {
    pub title: String,
    pub creators: Vec<String>,
    pub catalog_number: Option<String>,
    pub kind: String,
    pub confidence: f64,
    pub rationale: String,
}

impl WorkProposal {
    pub fn validate(&self) -> Result<()> {
        identity(self).map(|_| ())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResolution {
    pub track_id: String,
    pub work: Option<Work>,
    pub status: String,
    pub reason: String,
    pub evaluated_at: i64,
}

pub(super) fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS work_scan_v1 (id INTEGER PRIMARY KEY CHECK(id=1), scan_offset INTEGER NOT NULL);
        INSERT OR IGNORE INTO work_scan_v1 VALUES(1,0);
        CREATE TABLE IF NOT EXISTS works_v1 (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, creators_json TEXT NOT NULL,
            catalog_number TEXT, kind TEXT NOT NULL, identity_key TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS work_resolutions_v1 (
            track_id TEXT PRIMARY KEY, work_id TEXT REFERENCES works_v1(id),
            status TEXT NOT NULL CHECK(status IN ('linked', 'created', 'unresolved')),
            reason TEXT NOT NULL, evidence_json TEXT NOT NULL, evaluated_at INTEGER NOT NULL,
            enriched_at INTEGER NOT NULL, last_verified_at INTEGER, source_status TEXT NOT NULL,
            CHECK((status = 'unresolved') = (work_id IS NULL))
        );
        CREATE INDEX IF NOT EXISTS idx_work_resolutions_work ON work_resolutions_v1(work_id);",
    )?;
    Ok(())
}

// Deliberately conservative: preserve accents, punctuation, movement numbers and
// catalog suffixes. False negatives can be reviewed; false merges spread attachments.
fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn identity(proposal: &WorkProposal) -> Result<String> {
    ensure!(!proposal.title.trim().is_empty(), "missing work title");
    ensure!(
        proposal.title.len() <= 500
            && proposal.creators.len() <= 30
            && proposal.creators.iter().all(|n| n.len() <= 200),
        "work identity too large"
    );
    ensure!(
        proposal.confidence.is_finite() && (0.9..=1.0).contains(&proposal.confidence),
        "insufficient work confidence"
    );
    ensure!(
        !proposal.rationale.trim().is_empty(),
        "missing identity rationale"
    );
    ensure!(
        matches!(
            proposal.kind.as_str(),
            "song" | "composition" | "movement" | "aria" | "standard"
        ),
        "unsupported or composite work kind"
    );
    ensure!(
        !proposal.creators.is_empty()
            && proposal.creators.iter().all(|name| !name.trim().is_empty()),
        "work creators are unknown"
    );
    ensure!(
        proposal.creators.iter().all(|name| !matches!(
            normalize(name).as_str(),
            "unknown" | "anonymous" | "traditional" | "null"
        )),
        "work creators are unknown"
    );
    let mut creators: Vec<_> = proposal
        .creators
        .iter()
        .map(|name| normalize(name))
        .collect();
    creators.sort();
    creators.dedup();
    // JSON tuple avoids separator collisions. Kind is descriptive, not identity:
    // the same song may also be described as a jazz standard.
    Ok(serde_json::to_string(&(
        normalize(&proposal.title),
        creators,
        proposal
            .catalog_number
            .as_deref()
            .map(normalize)
            .filter(|s| !s.is_empty()),
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrichment_store::EnrichmentStore;

    fn setup() -> (SqliteEnrichmentStore, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = SqliteEnrichmentStore::new(
            tmp.path().join("enrichment.db"),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        (store, tmp)
    }

    fn proposal(title: &str, creator: &str) -> WorkProposal {
        WorkProposal {
            title: title.into(),
            creators: vec![creator.into()],
            catalog_number: None,
            kind: "song".into(),
            confidence: 0.95,
            rationale: "Recognized composition and songwriter".into(),
        }
    }

    #[test]
    fn work_resolution_reuses_identity_across_performances_and_retries() {
        let (store, _tmp) = setup();
        let original = store
            .resolve_track_work(
                "original",
                Some(&proposal("Example Song", "Writer One")),
                &serde_json::json!({}),
                "",
            )
            .unwrap();
        let cover = store
            .resolve_track_work(
                "cover",
                Some(&proposal(" example  song ", "WRITER ONE")),
                &serde_json::json!({}),
                "",
            )
            .unwrap();
        assert_eq!(original.status, "created");
        assert_eq!(cover.status, "linked");
        assert_eq!(original.work, cover.work);
        let retry = store
            .resolve_track_work(
                "cover",
                Some(&proposal("Wrong Song", "Wrong Writer")),
                &serde_json::json!({}),
                "",
            )
            .unwrap();
        assert_eq!(cover.work, retry.work);
        let work = cover.work.unwrap();
        assert_eq!(
            store.list_work_track_ids(&work.id, 100, 0).unwrap(),
            vec!["cover", "original"]
        );
        assert_eq!(
            store.list_work_track_ids(&work.id, 1, 1).unwrap(),
            vec!["original"]
        );
        assert_eq!(store.search_works("Example", 25).unwrap().len(), 1);
    }

    #[test]
    fn work_resolution_keeps_namesakes_and_movements_separate() {
        let (store, _tmp) = setup();
        let inputs = [
            ("a", "Same Title", "Writer A"),
            ("b", "Same Title", "Writer B"),
            ("c", "Suite: I. Allegro", "Writer A"),
            ("d", "Suite: II. Allegro", "Writer A"),
        ];
        let mut ids = std::collections::HashSet::new();
        for (track, title, creator) in inputs {
            let result = store
                .resolve_track_work(
                    track,
                    Some(&proposal(title, creator)),
                    &serde_json::json!({}),
                    "",
                )
                .unwrap();
            assert!(ids.insert(result.work.unwrap().id));
        }
    }

    #[test]
    fn work_resolution_abstains_on_uncertain_or_composite_identity() {
        let (store, _tmp) = setup();
        for case in ["confidence", "creator", "medley", "reason", "nan", "empty"] {
            let mut p = proposal("Song", "Writer");
            match case {
                "confidence" => p.confidence = 0.89,
                "creator" => p.creators.clear(),
                "medley" => p.kind = "medley".into(),
                "reason" => p.rationale.clear(),
                "nan" => p.confidence = f64::NAN,
                "empty" => p.title = " ".into(),
                _ => unreachable!(),
            }
            let r = store
                .resolve_track_work(case, Some(&p), &serde_json::json!({"case":case}), "")
                .unwrap();
            assert_eq!(r.status, "unresolved");
            assert!(r.work.is_none());
        }
        assert!(store.search_works("Song", 100).unwrap().is_empty());
    }

    #[test]
    fn work_resolution_migration_is_repeatable_and_queue_is_independent() {
        let (store, tmp) = setup();
        assert!(store
            .enqueue_enrichment_if_missing_or_stale("work_resolution", "t", "test", 4, 86400)
            .unwrap());
        let queue = store
            .claim_enrichment_queue_batch_for_types(10, &["work_resolution".into()])
            .unwrap();
        assert_eq!(queue.len(), 1);
        store
            .resolve_track_work(
                "t",
                None,
                &serde_json::json!({"response":"unknown"}),
                "Not recognized",
            )
            .unwrap();
        store.complete_enrichment_queue_item(queue[0].id).unwrap();
        assert!(!store
            .enqueue_enrichment_if_missing_or_stale("work_resolution", "t", "test", 4, 86400)
            .unwrap());
        assert!(store.get_track_enrichment_v1("t").unwrap().is_none());
        drop(store);
        let reopened = SqliteEnrichmentStore::new(
            tmp.path().join("enrichment.db"),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        assert_eq!(
            reopened.get_work_resolution("t").unwrap().unwrap().reason,
            "Not recognized"
        );
    }

    #[test]
    fn work_resolution_failure_rolls_back_new_work() {
        let (store, _tmp) = setup();
        store.write_conn.lock().unwrap().execute_batch("CREATE TRIGGER reject_link BEFORE INSERT ON work_resolutions_v1 BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(store
            .resolve_track_work(
                "t",
                Some(&proposal("Song", "Writer")),
                &serde_json::json!({}),
                ""
            )
            .is_err());
        assert!(store.search_works("Song", 100).unwrap().is_empty());
    }
}

fn work_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Work> {
    let creators: String = row.get(2)?;
    Ok(Work {
        id: row.get(0)?,
        title: row.get(1)?,
        creators: serde_json::from_str(&creators).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        catalog_number: row.get(3)?,
        kind: row.get(4)?,
        created_at: row.get(5)?,
    })
}

impl SqliteEnrichmentStore {
    pub(super) fn read_work(&self, id: &str) -> Result<Option<Work>> {
        Ok(self.read_conn.lock().unwrap().query_row(
            "SELECT id,title,creators_json,catalog_number,kind,created_at FROM works_v1 WHERE id=?1",
            [id], work_from_row,
        ).optional()?)
    }

    pub(super) fn find_works(&self, query: &str, limit: usize) -> Result<Vec<Work>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,title,creators_json,catalog_number,kind,created_at FROM works_v1 WHERE instr(lower(title),lower(?1)) > 0 ORDER BY title,id LIMIT ?2")?;
        let rows = stmt.query_map(params![query.trim(), limit.min(100) as i64], work_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(super) fn read_work_resolution(&self, track_id: &str) -> Result<Option<WorkResolution>> {
        let row: Option<(Option<String>, String, String, i64)> = self.read_conn.lock().unwrap().query_row(
            "SELECT work_id,status,reason,evaluated_at FROM work_resolutions_v1 WHERE track_id=?1", [track_id],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        ).optional()?;
        row.map(|(id, status, reason, evaluated_at)| {
            Ok(WorkResolution {
                track_id: track_id.to_owned(),
                work: id.map(|id| self.read_work(&id)).transpose()?.flatten(),
                status,
                reason,
                evaluated_at,
            })
        })
        .transpose()
    }

    pub(super) fn work_track_ids(
        &self,
        work_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<String>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT track_id FROM work_resolutions_v1 WHERE work_id=?1 ORDER BY track_id LIMIT ?2 OFFSET ?3")?;
        let rows = stmt.query_map(
            params![work_id, limit.min(100) as i64, offset as i64],
            |r| r.get(0),
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(super) fn resolve_work(
        &self,
        track_id: &str,
        proposal: Option<&WorkProposal>,
        evidence: &serde_json::Value,
        unresolved_reason: &str,
    ) -> Result<WorkResolution> {
        ensure!(!track_id.trim().is_empty(), "missing track id");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Retry after a successful write must not change an established identity.
        let existing: Option<Option<String>> = tx
            .query_row(
                "SELECT work_id FROM work_resolutions_v1 WHERE track_id=?1",
                [track_id],
                |r| r.get(0),
            )
            .optional()?;
        if existing.flatten().is_some() {
            drop(tx);
            drop(conn);
            return self
                .read_work_resolution(track_id)?
                .ok_or_else(|| anyhow::anyhow!("resolution disappeared"));
        }
        let mut work_id = None;
        let mut status = "unresolved";
        let mut reason = unresolved_reason.to_owned();
        if let Some(proposal) = proposal {
            match identity(proposal) {
                Ok(key) => {
                    let existing: Option<String> = tx
                        .query_row(
                            "SELECT id FROM works_v1 WHERE identity_key=?1",
                            [&key],
                            |r| r.get(0),
                        )
                        .optional()?;
                    if let Some(id) = existing {
                        work_id = Some(id);
                        status = "linked";
                    } else {
                        let id = uuid::Uuid::new_v4().to_string();
                        tx.execute("INSERT INTO works_v1(id,title,creators_json,catalog_number,kind,identity_key,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                            params![id,proposal.title.trim(),serde_json::to_string(&proposal.creators)?,proposal.catalog_number,proposal.kind,key,now])?;
                        work_id = Some(id);
                        status = "created";
                    }
                    reason = proposal.rationale.clone();
                }
                Err(err) => reason = err.to_string(),
            }
        }
        tx.execute("INSERT INTO work_resolutions_v1(track_id,work_id,status,reason,evidence_json,evaluated_at,enriched_at,last_verified_at,source_status)
            VALUES(?1,?2,?3,?4,?5,?6,?6,?6,'llm_work_v1')
            ON CONFLICT(track_id) DO UPDATE SET work_id=excluded.work_id,status=excluded.status,reason=excluded.reason,evidence_json=excluded.evidence_json,evaluated_at=excluded.evaluated_at,enriched_at=excluded.enriched_at,last_verified_at=excluded.last_verified_at",
            params![track_id,work_id,status,reason,serde_json::to_string(evidence)?,now])?;
        tx.commit()?;
        drop(conn);
        self.read_work_resolution(track_id)?
            .ok_or_else(|| anyhow::anyhow!("resolution disappeared"))
    }
}
