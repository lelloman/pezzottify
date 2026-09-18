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
    pub wikidata_id: Option<String>,
    pub musicbrainz_id: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
pub struct WorkPresentation {
    pub creator_mbids: Vec<String>,
    pub composition_year: Option<String>,
}

impl WorkPresentation {
    fn from_evidence(evidence: &serde_json::Value) -> Self {
        let mut result = Self::default();
        let mut years = Vec::new();
        for relation in evidence["artist_relations"].as_array().into_iter().flatten() {
            let role = relation["type"].as_str().unwrap_or_default();
            if !matches!(role, "composer" | "writer" | "lyricist" | "librettist") {
                continue;
            }
            if let Some(mbid) = relation["artist"]["mbid"].as_str() {
                if !mbid.is_empty() && !result.creator_mbids.iter().any(|id| id == mbid) {
                    result.creator_mbids.push(mbid.to_owned());
                }
            }
            // Writing dates belong to the composition, never to its recordings
            // or the timestamp when this Work was imported into our database.
            if matches!(role, "composer" | "writer") {
                for field in ["begin_date", "end_date"] {
                    if let Some(year) = relation[field].get(0).and_then(|v| v.as_i64()) {
                        if (1..=9999).contains(&year) {
                            years.push(year);
                        }
                    }
                }
            }
        }
        if let (Some(first), Some(last)) = (years.iter().min(), years.iter().max()) {
            result.composition_year = Some(if first == last {
                first.to_string()
            } else {
                format!("{first}–{last}")
            });
        }
        result
    }
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
    conn.execute_batch("CREATE TABLE IF NOT EXISTS work_external_ids_v1 (
        provider TEXT NOT NULL, external_id TEXT NOT NULL, work_id TEXT NOT NULL REFERENCES works_v1(id),
        PRIMARY KEY(provider,external_id), UNIQUE(work_id,provider)
    );")?;
    conn.execute_batch(include_str!("work_graph.sql"))?;
    // Keep search normalization separate from conservative Work identity matching.
    // A savepoint makes the initial index creation/backfill atomic, including upgrades.
    conn.execute_batch("SAVEPOINT work_search_schema")?;
    let search_setup = (|| -> Result<()> {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='work_search_v1')",
            [],
            |row| row.get(0),
        )?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS work_search_v1 USING fts5(
                title, creators_json, content='works_v1', content_rowid='rowid',
                tokenize='unicode61 remove_diacritics 2'
            );
            CREATE TRIGGER IF NOT EXISTS work_search_insert AFTER INSERT ON works_v1 BEGIN
                INSERT INTO work_search_v1(rowid,title,creators_json)
                VALUES(new.rowid,new.title,new.creators_json);
            END;
            CREATE TRIGGER IF NOT EXISTS work_search_delete AFTER DELETE ON works_v1 BEGIN
                INSERT INTO work_search_v1(work_search_v1,rowid,title,creators_json)
                VALUES('delete',old.rowid,old.title,old.creators_json);
            END;
            CREATE TRIGGER IF NOT EXISTS work_search_update AFTER UPDATE ON works_v1 BEGIN
                INSERT INTO work_search_v1(work_search_v1,rowid,title,creators_json)
                VALUES('delete',old.rowid,old.title,old.creators_json);
                INSERT INTO work_search_v1(rowid,title,creators_json)
                VALUES(new.rowid,new.title,new.creators_json);
            END;",
        )?;
        if !exists {
            conn.execute(
                "INSERT INTO work_search_v1(work_search_v1) VALUES('rebuild')",
                [],
            )?;
        }
        Ok(())
    })();
    if let Err(error) = search_setup {
        conn.execute_batch("ROLLBACK TO work_search_schema; RELEASE work_search_schema")?;
        return Err(error);
    }
    conn.execute_batch("RELEASE work_search_schema")?;
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

    #[test]
    fn musicbrainz_work_ids_are_stable_and_namesakes_do_not_merge() {
        let (store, _tmp) = setup();
        let evidence = |id: &str| serde_json::json!({"selected_source":{"musicbrainz_id":id}});
        let id = "00000000-0000-0000-0000-000000000001";
        let first = store
            .resolve_track_work("a", Some(&proposal("Song", "Writer")), &evidence(id), "")
            .unwrap()
            .work
            .unwrap();
        let renamed = store
            .resolve_track_work(
                "b",
                Some(&proposal("Canonical Song", "Writer")),
                &evidence(id),
                "",
            )
            .unwrap()
            .work
            .unwrap();
        assert_eq!(first.id, renamed.id);
        assert_eq!(first.musicbrainz_id.as_deref(), Some(id));
        let other = store
            .resolve_track_work(
                "c",
                Some(&proposal("Song", "Writer")),
                &evidence("00000000-0000-0000-0000-000000000002"),
                "",
            )
            .unwrap()
            .work
            .unwrap();
        assert_ne!(first.id, other.id);
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
    fn work_presentation_uses_creator_identity_and_writing_dates_only() {
        let evidence = serde_json::json!({"artist_relations": [
            {"type":"composer", "artist":{"mbid":"composer"}, "begin_date":[1829, null, null], "end_date":[1832, null, null]},
            {"type":"writer", "artist":{"mbid":"composer"}, "begin_date":[1830, 1, 1]},
            {"type":"lyricist", "artist":{"mbid":"lyricist"}, "begin_date":[1900, 1, 1]},
            {"type":"arranger", "artist":{"mbid":"arranger"}, "begin_date":[2000, 1, 1]}
        ]});
        let presentation = WorkPresentation::from_evidence(&evidence);
        assert_eq!(presentation.creator_mbids, ["composer", "lyricist"]);
        assert_eq!(presentation.composition_year.as_deref(), Some("1829–1832"));
        assert_eq!(WorkPresentation::from_evidence(&serde_json::json!({})), WorkPresentation::default());
        let single = serde_json::json!({"artist_relations": [
            {"type":"composer", "artist":{"mbid":"composer"}, "begin_date":[1832, null, null], "end_date":[1832, null, null]}
        ]});
        assert_eq!(WorkPresentation::from_evidence(&single).composition_year.as_deref(), Some("1832"));
        let unknown = serde_json::json!({"artist_relations": [
            {"type":"composer", "artist":{"mbid":"composer"}, "begin_date":[null, null, null], "end_date":[0, null, null]}
        ]});
        assert_eq!(WorkPresentation::from_evidence(&unknown).composition_year, None);
    }

    #[test]
    fn work_presentation_reads_imported_evidence_and_defaults_when_absent() {
        let (store, _tmp) = setup();
        let resolved = store.resolve_track_work("track", Some(&proposal("Etude", "Composer")), &serde_json::json!({}), "").unwrap();
        let id = resolved.work.unwrap().id;
        assert_eq!(store.work_presentation(&id).unwrap(), WorkPresentation::default());
        store.write_conn.lock().unwrap().execute(
            "INSERT INTO work_source_evidence_v1 VALUES ('musicbrainz', 'work-mbid', ?1, 'snapshot', ?2, 0)",
            params![id, serde_json::json!({"artist_relations":[{"type":"composer","artist":{"mbid":"artist-mbid"},"begin_date":[1832,null,null]}]}).to_string()],
        ).unwrap();
        let presentation = store.work_presentation(&id).unwrap();
        assert_eq!(presentation.creator_mbids, ["artist-mbid"]);
        assert_eq!(presentation.composition_year.as_deref(), Some("1832"));
    }

    #[test]
    fn work_search_matches_title_and_creators_with_accents_and_prefixes() {
        let (store, _tmp) = setup();
        for (track, title, creator) in [
            ("a", "Étude Op. 10 No. 3", "Frédéric Chopin"),
            ("b", "Étude Op. 25 No. 1", "Frédéric Chopin"),
            ("c", "Étude", "Franz Liszt"),
            ("d", "Nocturne", "Frédéric Chopin"),
        ] {
            store
                .resolve_track_work(
                    track,
                    Some(&proposal(title, creator)),
                    &serde_json::json!({}),
                    "",
                )
                .unwrap();
        }
        for query in [
            "chopin etude",
            "ETUDE CHOPIN",
            "  chopin   étu  ",
            "frederic etude",
            "chopin e\u{0301}tude",
            "\"chopin\" (etude)",
        ] {
            let works = store.search_works(query, 25).unwrap();
            assert_eq!(works.len(), 2, "{query}");
            assert!(works
                .iter()
                .all(|work| work.creators == ["Frédéric Chopin"]));
        }
        assert_eq!(store.search_works("chopin etude 25", 25).unwrap().len(), 1);
        assert_eq!(store.search_works("etude", 25).unwrap().len(), 3);
        assert_eq!(store.search_works("chopin", 25).unwrap().len(), 3);
        assert_eq!(store.search_works("chopin etude", 1).unwrap().len(), 1);
        for query in ["", "  ", "* () :", "chopin missing", "chopin OR liszt"] {
            assert!(store.search_works(query, 25).unwrap().is_empty(), "{query}");
        }
    }

    #[test]
    fn work_search_backfills_existing_works_and_tracks_changes() {
        let (store, _tmp) = setup();
        store
            .resolve_track_work(
                "a",
                Some(&proposal("Étude", "Chopin")),
                &serde_json::json!({}),
                "",
            )
            .unwrap();
        {
            let conn = store.write_conn.lock().unwrap();
            // Simulate a database created before the search index existed.
            conn.execute_batch("DROP TRIGGER work_search_insert; DROP TRIGGER work_search_update; DROP TRIGGER work_search_delete; DROP TABLE work_search_v1;").unwrap();
            super::create_schema(&conn).unwrap();
            super::create_schema(&conn).unwrap();
        }
        assert_eq!(store.search_works("chopin etude", 25).unwrap().len(), 1);
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "UPDATE works_v1 SET title='Nocturne', creators_json='[\"Another Writer\"]'",
                [],
            )
            .unwrap();
        }
        assert!(store.search_works("chopin etude", 25).unwrap().is_empty());
        assert_eq!(store.search_works("writer nocturne", 25).unwrap().len(), 1);
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute("DELETE FROM work_resolutions_v1", []).unwrap();
            conn.execute("DELETE FROM works_v1", []).unwrap();
        }
        assert!(store
            .search_works("writer nocturne", 25)
            .unwrap()
            .is_empty());
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
    fn wikidata_work_identity_survives_label_changes_and_separates_external_namesakes() {
        let (store, tmp) = setup();
        let evidence = |qid: &str| serde_json::json!({"selected_source":{"qid":qid}});
        let first = store
            .resolve_track_work("a", Some(&proposal("Title", "Writer")), &evidence("Q1"), "")
            .unwrap();
        let renamed = store
            .resolve_track_work(
                "b",
                Some(&proposal("Localized title", "Writer")),
                &evidence("Q1"),
                "",
            )
            .unwrap();
        assert_eq!(first.work, renamed.work);
        assert_eq!(
            first.work.as_ref().unwrap().wikidata_id.as_deref(),
            Some("Q1")
        );
        let namesake = store
            .resolve_track_work("c", Some(&proposal("Title", "Writer")), &evidence("Q2"), "")
            .unwrap();
        assert_ne!(
            first.work.as_ref().unwrap().id,
            namesake.work.as_ref().unwrap().id
        );
        assert_eq!(store.search_works("Title", 25).unwrap().len(), 2);
        drop(store);
        let reopened = SqliteEnrichmentStore::new(
            tmp.path().join("enrichment.db"),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        assert_eq!(
            reopened
                .get_work(&first.work.unwrap().id)
                .unwrap()
                .unwrap()
                .wikidata_id
                .as_deref(),
            Some("Q1")
        );
    }

    #[test]
    fn wikidata_work_can_add_a_reference_to_an_existing_inferred_identity() {
        let (store, _tmp) = setup();
        let first = store
            .resolve_track_work(
                "a",
                Some(&proposal("Song", "Writer")),
                &serde_json::json!({}),
                "",
            )
            .unwrap();
        let linked = store
            .resolve_track_work(
                "b",
                Some(&proposal("Song", "Writer")),
                &serde_json::json!({"selected_source":{"qid":"Q1"}}),
                "",
            )
            .unwrap();
        assert_eq!(first.work.unwrap().id, linked.work.as_ref().unwrap().id);
        assert_eq!(linked.work.unwrap().wikidata_id.as_deref(), Some("Q1"));
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
        wikidata_id: row.get(6)?,
        musicbrainz_id: row.get(7)?,
    })
}

impl SqliteEnrichmentStore {
    pub(super) fn read_work_presentation(&self, id: &str) -> Result<WorkPresentation> {
        let evidence: Option<String> = self.read_conn.lock().unwrap().query_row(
            "SELECT evidence_json FROM work_source_evidence_v1 WHERE work_id=?1 AND provider='musicbrainz'",
            [id], |row| row.get(0),
        ).optional()?;
        match evidence {
            Some(json) => Ok(WorkPresentation::from_evidence(&serde_json::from_str(&json)?)),
            None => Ok(WorkPresentation::default()),
        }
    }
    pub(super) fn read_work(&self, id: &str) -> Result<Option<Work>> {
        Ok(self.read_conn.lock().unwrap().query_row(
            "SELECT id,title,creators_json,catalog_number,kind,created_at,(SELECT external_id FROM work_external_ids_v1 WHERE work_id=works_v1.id AND provider='wikidata'),(SELECT external_id FROM work_external_ids_v1 WHERE work_id=works_v1.id AND provider='musicbrainz') FROM works_v1 WHERE id=?1",
            [id], work_from_row,
        ).optional()?)
    }

    pub(super) fn find_works(&self, query: &str, limit: usize) -> Result<Vec<Work>> {
        // Treat user input as literal terms, never as FTS operators. Prefix matching
        // supports incomplete words while requiring every term across title/creators.
        let query = query
            .split(|c: char| !c.is_alphanumeric() && !matches!(c, '\u{0300}'..='\u{036f}'))
            .filter(|term| !term.is_empty())
            .map(|term| format!("\"{term}\"*"))
            .collect::<Vec<_>>()
            .join(" AND ");
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,works_v1.title,works_v1.creators_json,catalog_number,kind,created_at,(SELECT external_id FROM work_external_ids_v1 WHERE work_id=works_v1.id AND provider='wikidata'),(SELECT external_id FROM work_external_ids_v1 WHERE work_id=works_v1.id AND provider='musicbrainz') FROM works_v1 JOIN work_search_v1 ON work_search_v1.rowid=works_v1.rowid WHERE work_search_v1 MATCH ?1 ORDER BY bm25(work_search_v1),works_v1.title,id LIMIT ?2")?;
        let rows = stmt.query_map(params![query, limit.min(100) as i64], work_from_row)?;
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
        // This evidence is assembled by the server after validating the model's
        // citation against fetched facts; it is not a model-supplied payload.
        let wikidata_id = evidence
            .pointer("/selected_source/qid")
            .and_then(|v| v.as_str());
        if let Some(qid) = wikidata_id {
            ensure!(
                qid.strip_prefix('Q').is_some_and(|n| !n.is_empty()
                    && n.len() <= 20
                    && n.bytes().all(|c| c.is_ascii_digit())),
                "invalid Wikidata identifier"
            );
        }
        let musicbrainz_id = evidence
            .pointer("/selected_source/musicbrainz_id")
            .and_then(|v| v.as_str());
        if let Some(id) = musicbrainz_id {
            ensure!(
                uuid::Uuid::parse_str(id).is_ok(),
                "invalid MusicBrainz identifier"
            );
        }
        ensure!(
            wikidata_id.is_none() || musicbrainz_id.is_none(),
            "multiple unverified source identities"
        );
        let (source_provider, source_id) = if let Some(id) = wikidata_id {
            ("wikidata", Some(id))
        } else {
            ("musicbrainz", musicbrainz_id)
        };
        if let Some(proposal) = proposal {
            match identity(proposal) {
                Ok(mut key) => {
                    let by_source: Option<String> = if let Some(qid) = source_id {
                        tx.query_row("SELECT work_id FROM work_external_ids_v1 WHERE provider=?1 AND external_id=?2", params![source_provider,qid], |r| r.get(0)).optional()?
                    } else {
                        None
                    };
                    let mut existing: Option<String> = tx
                        .query_row(
                            "SELECT id FROM works_v1 WHERE identity_key=?1",
                            [&key],
                            |r| r.get(0),
                        )
                        .optional()?;
                    if let Some(source_work) = by_source {
                        existing = Some(source_work);
                    } else if let (Some(id), Some(qid)) = (existing.as_ref(), source_id) {
                        let other_source: Option<String> = tx.query_row("SELECT external_id FROM work_external_ids_v1 WHERE provider=?1 AND work_id=?2", params![source_provider,id], |r| r.get(0)).optional()?;
                        if other_source.is_some() || tx.query_row("SELECT EXISTS(SELECT 1 FROM work_external_ids_v1 WHERE work_id=?1)", [id], |r| r.get::<_, bool>(0))? {
                            // Different externally identified works can share a
                            // title and creator. Do not merge them on text alone.
                            existing = None;
                            key = format!("{source_provider}:{qid}");
                        }
                    }
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
                    if let Some(qid) = source_id {
                        tx.execute("INSERT OR IGNORE INTO work_external_ids_v1(provider,external_id,work_id) VALUES(?1,?2,?3)",params![source_provider,qid,work_id])?;
                    }
                }
                Err(err) => reason = err.to_string(),
            }
        }
        let source_status = if work_id.is_some() && source_id.is_some() {
            if source_provider == "wikidata" {
                "wikidata_supported_v1"
            } else {
                "musicbrainz_supported_v1"
            }
        } else if work_id.is_none() && evidence["prompt_version"] == "work-resolution-v3-sources" {
            "source_unresolved_v3"
        } else {
            "llm_work_v2"
        };
        tx.execute("INSERT INTO work_resolutions_v1(track_id,work_id,status,reason,evidence_json,evaluated_at,enriched_at,last_verified_at,source_status)
            VALUES(?1,?2,?3,?4,?5,?6,?6,?6,?7)
            ON CONFLICT(track_id) DO UPDATE SET work_id=excluded.work_id,status=excluded.status,reason=excluded.reason,evidence_json=excluded.evidence_json,evaluated_at=excluded.evaluated_at,enriched_at=excluded.enriched_at,last_verified_at=excluded.last_verified_at,source_status=excluded.source_status",
            params![track_id,work_id,status,reason,serde_json::to_string(evidence)?,now,source_status])?;
        tx.commit()?;
        drop(conn);
        self.read_work_resolution(track_id)?
            .ok_or_else(|| anyhow::anyhow!("resolution disappeared"))
    }
}
