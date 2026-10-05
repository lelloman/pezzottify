//! Durable retry budgets and per-attempt evidence. Claims do not consume attempts.
use super::{EnrichmentQueueItemV1, EnrichmentStore, SqliteEnrichmentStore};
use anyhow::{ensure, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("SAVEPOINT enrichment_retry_schema")?;
    let result = (|| -> Result<()> {
        let has_counters: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('enrichment_queue_v1') WHERE name='normal_attempts')",
            [], |r| r.get(0))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS enrichment_attempts_v1 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue_id INTEGER NOT NULL REFERENCES enrichment_queue_v1(id),
                cycle INTEGER NOT NULL,
                phase TEXT NOT NULL,
                attempt INTEGER NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                outcome TEXT NOT NULL,
                error TEXT,
                diagnostics_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_enrichment_attempts_queue ON enrichment_attempts_v1(queue_id,id);
            CREATE TABLE IF NOT EXISTS work_link_quarantine_v1 (
                track_id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                original_status TEXT NOT NULL,
                original_source_status TEXT NOT NULL,
                original_reason TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                evaluated_at INTEGER NOT NULL,
                enriched_at INTEGER NOT NULL,
                last_verified_at INTEGER,
                quarantined_at INTEGER NOT NULL
            );"
        )?;
        if !has_counters {
            conn.execute_batch(
                "ALTER TABLE enrichment_queue_v1 ADD COLUMN normal_attempts INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE enrichment_queue_v1 ADD COLUMN agent_attempts INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE enrichment_queue_v1 ADD COLUMN cycle INTEGER NOT NULL DEFAULT 1;
                INSERT INTO enrichment_attempts_v1(queue_id,cycle,phase,attempt,started_at,finished_at,outcome,error,diagnostics_json)
                SELECT id,1,'legacy',attempts,created_at,updated_at,'migrated',last_error,
                    json_object('lifetime_claims',attempts,'status',status,'note','Legacy counts are claims, not consecutive failures')
                FROM enrichment_queue_v1 WHERE attempts>0;
                UPDATE enrichment_queue_v1 SET
                    normal_attempts=CASE WHEN status='completed' THEN 0 ELSE MIN(attempts,12) END,
                    status=CASE WHEN status='failed' THEN 'failed_enrichment' WHEN status='running' THEN 'queued' ELSE status END,
                    stage=CASE WHEN status='failed' THEN 'failed_enrichment'
                        WHEN status!='completed' AND attempts>=12 THEN 'agent'
                        WHEN status!='completed' THEN 'normal' ELSE stage END;"
            )?;
        }
        // Legacy title-only model selections are unverified attachments, not
        // proven incorrect Works. Archive every original field before detaching.
        conn.execute_batch(
            "INSERT OR IGNORE INTO work_link_quarantine_v1
                SELECT track_id,work_id,status,source_status,reason,evidence_json,
                    evaluated_at,enriched_at,last_verified_at,CAST(strftime('%s','now') AS INTEGER)
                FROM work_resolutions_v1
                WHERE work_id IS NOT NULL AND source_status='wikidata_supported_v1';
            INSERT OR IGNORE INTO enrichment_queue_v1(entity_type,entity_id,status,stage,created_at,updated_at)
                SELECT 'work_resolution',track_id,'completed','completed',evaluated_at,evaluated_at
                FROM work_resolutions_v1 WHERE work_id IS NOT NULL AND source_status='wikidata_supported_v1';
            UPDATE enrichment_queue_v1 SET status='failed_enrichment',stage='failed_enrichment',
                next_attempt_at=NULL,completed_at=CAST(strftime('%s','now') AS INTEGER),
                updated_at=CAST(strftime('%s','now') AS INTEGER),
                last_error='Unverified legacy Work link quarantined; manual retry required'
                WHERE entity_type='work_resolution' AND entity_id IN (
                    SELECT track_id FROM work_resolutions_v1 WHERE work_id IS NOT NULL AND source_status='wikidata_supported_v1');
            UPDATE work_resolutions_v1 SET work_id=NULL,status='unresolved',
                source_status='needs_review_v1',last_verified_at=NULL,
                reason='Unverified legacy Work link quarantined; original evidence retained'
                WHERE work_id IS NOT NULL AND source_status='wikidata_supported_v1';"
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        conn.execute_batch("ROLLBACK TO enrichment_retry_schema; RELEASE enrichment_retry_schema")?;
        return Err(error);
    }
    conn.execute_batch("RELEASE enrichment_retry_schema")?;
    Ok(())
}

impl SqliteEnrichmentStore {
    pub(super) fn begin_attempt(&self, id: i64) -> Result<EnrichmentQueueItemV1> {
        let now = super::store::now_unix();
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction()?;
        let (kind, entity, phase, cycle, number): (String, String, String, i64, i64) = tx.query_row(
            "SELECT entity_type,entity_id,CASE WHEN normal_attempts<12 THEN 'normal' ELSE 'agent' END,
                cycle,CASE WHEN normal_attempts<12 THEN normal_attempts+1 ELSE agent_attempts+1 END
             FROM enrichment_queue_v1 WHERE id=?1 AND status='running'
                AND (normal_attempts<12 OR agent_attempts<3)
                AND NOT EXISTS(SELECT 1 FROM enrichment_attempts_v1 WHERE queue_id=?1 AND outcome='running')",
            [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        tx.execute(
            "UPDATE enrichment_queue_v1 SET attempts=attempts+1,stage=?2,
                normal_attempts=normal_attempts+CASE WHEN ?2='normal' THEN 1 ELSE 0 END,
                agent_attempts=agent_attempts+CASE WHEN ?2='agent' THEN 1 ELSE 0 END,
                started_at=?3,updated_at=?3 WHERE id=?1",
            params![id, phase, now],
        )?;
        tx.execute(
            "INSERT INTO enrichment_attempts_v1(queue_id,cycle,phase,attempt,started_at,outcome)
             VALUES(?1,?2,?3,?4,?5,'running')",
            params![id, cycle, phase, number, now],
        )?;
        tx.commit()?;
        drop(conn);
        self.get_enrichment_queue_item(&kind, &entity)?
            .ok_or_else(|| anyhow::anyhow!("queue item disappeared"))
    }

    pub(super) fn history(&self, id: i64) -> Result<Vec<Value>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT cycle,phase,attempt,started_at,finished_at,outcome,error,diagnostics_json
             FROM enrichment_attempts_v1 WHERE queue_id=?1 ORDER BY id DESC LIMIT 50",
        )?;
        let rows = stmt.query_map([id], |r| {
            let diagnostics: Option<String> = r.get(7)?;
            Ok(json!({
                "cycle":r.get::<_,i64>(0)?, "phase":r.get::<_,String>(1)?,
                "attempt":r.get::<_,i64>(2)?, "started_at":r.get::<_,i64>(3)?,
                "finished_at":r.get::<_,Option<i64>>(4)?, "outcome":r.get::<_,String>(5)?,
                "error":r.get::<_,Option<String>>(6)?,
                "diagnostics":diagnostics.and_then(|s| serde_json::from_str::<Value>(&s).ok())
            }))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(super) fn diagnostics(&self, id: i64, value: &Value) -> Result<()> {
        let value = serde_json::to_string(value)?;
        ensure!(value.len() <= 65536, "attempt diagnostics too large");
        let changed = self.write_conn.lock().unwrap().execute(
            "UPDATE enrichment_attempts_v1 SET diagnostics_json=?2 WHERE queue_id=?1 AND outcome='running'",
            params![id,value])?;
        ensure!(changed == 1, "no active enrichment attempt");
        Ok(())
    }

    pub(super) fn finish_attempt(
        &self,
        id: i64,
        error: Option<&str>,
        retry: Option<i64>,
    ) -> Result<()> {
        let now = super::store::now_unix();
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE enrichment_attempts_v1 SET outcome=?2,error=?3,finished_at=?4 WHERE queue_id=?1 AND outcome='running'",
            params![id,if error.is_some() {"failed"} else {"succeeded"},error,now])?;
        if let Some(error) = error {
            tx.execute(
                "UPDATE enrichment_queue_v1 SET
                    status=CASE WHEN ?3 IS NULL OR agent_attempts>=3 THEN 'failed_enrichment' ELSE 'queued' END,
                    stage=CASE WHEN ?3 IS NULL OR agent_attempts>=3 THEN 'failed_enrichment'
                        WHEN normal_attempts>=12 THEN 'agent' ELSE 'normal' END,
                    next_attempt_at=CASE WHEN ?3 IS NULL OR agent_attempts>=3 THEN NULL ELSE ?4+?3 END,
                    completed_at=CASE WHEN ?3 IS NULL OR agent_attempts>=3 THEN ?4 ELSE NULL END,
                    updated_at=?4,last_error=?2
                 WHERE id=?1 AND status='running'",
                params![id,error,retry,now])?;
        } else {
            tx.execute(
                "UPDATE enrichment_queue_v1 SET status='completed',stage='completed',
                    completed_at=?2,updated_at=?2,next_attempt_at=NULL,last_error=NULL,
                    normal_attempts=0,agent_attempts=0
                 WHERE id=?1 AND status='running'",
                params![id, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(super) fn release_claim(&self, id: i64, reason: &str) -> Result<()> {
        let now = super::store::now_unix();
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE enrichment_attempts_v1 SET outcome='interrupted',error=?2,finished_at=?3
             WHERE queue_id=?1 AND outcome='running'",
            params![id, reason, now],
        )?;
        tx.execute(
            "UPDATE enrichment_queue_v1 SET
                status=CASE WHEN agent_attempts>=3 THEN 'failed_enrichment' ELSE 'queued' END,
                stage=CASE WHEN agent_attempts>=3 THEN 'failed_enrichment' WHEN normal_attempts>=12 THEN 'agent' ELSE 'normal' END,
                started_at=NULL,next_attempt_at=NULL,updated_at=?3,last_error=?2,
                completed_at=CASE WHEN agent_attempts>=3 THEN ?3 ELSE NULL END
             WHERE id=?1 AND status='running'",params![id,reason,now])?;
        tx.commit()?;
        Ok(())
    }

    pub(super) fn manual_retry(&self, kind: &str, entity: &str) -> Result<bool> {
        ensure!(super::store::valid_entity_type(kind), "invalid entity type");
        Ok(self.write_conn.lock().unwrap().execute(
            "UPDATE enrichment_queue_v1 SET status='queued',stage='normal',normal_attempts=0,
                agent_attempts=0,cycle=cycle+1,started_at=NULL,completed_at=NULL,
                next_attempt_at=NULL,last_error=NULL,updated_at=?3,reason='manual_retry'
             WHERE entity_type=?1 AND entity_id=?2 AND status IN ('failed_enrichment','failed','completed')",
            params![kind,entity,super::store::now_unix()])? == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (SqliteEnrichmentStore, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let store = SqliteEnrichmentStore::new(
            temp.path().join("enrichment.db"),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        (store, temp)
    }
    fn enqueue(store: &SqliteEnrichmentStore, id: &str) {
        assert!(store
            .enqueue_enrichment_if_missing_or_stale("track", id, "listening", 10, 0)
            .unwrap());
    }
    fn attempt(store: &SqliteEnrichmentStore) -> EnrichmentQueueItemV1 {
        let claim = store.claim_enrichment_queue_batch(1).unwrap().remove(0);
        store.begin_enrichment_attempt(claim.id).unwrap()
    }

    #[test]
    fn enrichment_budget_is_twelve_normal_then_three_agent_then_terminal_across_restart() {
        let (mut store, temp) = setup();
        enqueue(&store, "track");
        for n in 1..=15 {
            let item = attempt(&store);
            assert_eq!(
                item.stage.as_deref(),
                Some(if n <= 12 { "normal" } else { "agent" })
            );
            assert_eq!(item.normal_attempts, n.min(12));
            assert_eq!(item.agent_attempts, (n - 12).max(0));
            store
                .record_enrichment_diagnostics(item.id, &json!({"attempt":n}))
                .unwrap();
            store
                .fail_enrichment_queue_item(item.id, "same unresolved identity", Some(0))
                .unwrap();
            // Neither fresh listens nor restarting can replenish the budget.
            assert!(!store
                .enqueue_enrichment_if_missing_or_stale("track", "track", "listening", 20, 0)
                .unwrap());
            drop(store);
            store = SqliteEnrichmentStore::new(
                temp.path().join("enrichment.db"),
                &crate::backup::DbRegistry::new(),
            )
            .unwrap();
        }
        assert!(store.claim_enrichment_queue_batch(1).unwrap().is_empty());
        let item = store
            .get_enrichment_queue_item("track", "track")
            .unwrap()
            .unwrap();
        assert_eq!(item.status, "failed_enrichment");
        assert!(item.next_attempt_at.is_none());
        assert_eq!(item.attempts, 15);
        let history = store.enrichment_attempt_history(item.id).unwrap();
        assert_eq!(history.len(), 15);
        assert_eq!(history[0]["diagnostics"]["attempt"], 15);
        assert_eq!(history[14]["phase"], "normal");
        assert!(store.retry_enrichment_manually("track", "track").unwrap());
        let next = attempt(&store);
        assert_eq!(next.cycle, 2);
        assert_eq!(next.normal_attempts, 1);
        assert_eq!(next.agent_attempts, 0);
        assert_eq!(store.enrichment_attempt_history(next.id).unwrap().len(), 16);
        store.complete_enrichment_queue_item(next.id).unwrap();
        let done = store
            .get_enrichment_queue_item("track", "track")
            .unwrap()
            .unwrap();
        assert_eq!((done.normal_attempts, done.agent_attempts), (0, 0));
    }

    #[test]
    fn discovery_preserves_error_backoff_and_permanent_failure() {
        let (store, _temp) = setup();
        enqueue(&store, "track");
        let item = attempt(&store);
        store
            .fail_enrichment_queue_item(item.id, "HTTP 503", Some(3600))
            .unwrap();
        let before = store
            .get_enrichment_queue_item("track", "track")
            .unwrap()
            .unwrap();
        assert!(!store
            .enqueue_enrichment_if_missing_or_stale("track", "track", "new listen", 100, 0)
            .unwrap());
        let after = store
            .get_enrichment_queue_item("track", "track")
            .unwrap()
            .unwrap();
        assert_eq!(before.last_error, after.last_error);
        assert_eq!(before.next_attempt_at, after.next_attempt_at);
        assert!(store.claim_enrichment_queue_batch(1).unwrap().is_empty());
        assert!(!store.retry_enrichment_manually("track", "track").unwrap());

        enqueue(&store, "missing");
        let missing = attempt(&store);
        store
            .fail_enrichment_queue_item(missing.id, "catalog track not found", None)
            .unwrap();
        assert!(!store
            .enqueue_enrichment_if_missing_or_stale("track", "missing", "new listen", 100, 0)
            .unwrap());
        assert_eq!(
            store
                .get_enrichment_queue_item("track", "missing")
                .unwrap()
                .unwrap()
                .status,
            "failed_enrichment"
        );
    }

    #[test]
    fn cancellation_does_not_charge_unstarted_claims_and_preserves_active_evidence() {
        let (store, _temp) = setup();
        enqueue(&store, "active");
        enqueue(&store, "waiting");
        let claimed = store.claim_enrichment_queue_batch(2).unwrap();
        assert!(claimed.iter().all(|q| q.attempts == 0));
        let active = store.begin_enrichment_attempt(claimed[0].id).unwrap();
        store
            .record_enrichment_diagnostics(active.id, &json!({"reason":"investigating"}))
            .unwrap();
        for item in &claimed {
            store.release_enrichment_claim(item.id, "shutdown").unwrap();
        }
        assert_eq!(
            store.enrichment_attempt_history(active.id).unwrap()[0]["outcome"],
            "interrupted"
        );
        assert!(store
            .enrichment_attempt_history(claimed[1].id)
            .unwrap()
            .is_empty());
        let waiting = store
            .get_enrichment_queue_item("track", &claimed[1].entity_id)
            .unwrap()
            .unwrap();
        assert_eq!(waiting.normal_attempts, 0);
        assert_eq!(waiting.attempts, 0);
        assert_eq!(store.claim_enrichment_queue_batch(2).unwrap().len(), 2);
    }

    #[test]
    fn migration_preserves_legacy_counts_and_does_not_grant_twelve_more_tries() {
        let conn = Connection::open_in_memory().unwrap();
        // Exact old queue shape, before the retry migration existed.
        conn.execute_batch("CREATE TABLE enrichment_queue_v1 (
            id INTEGER PRIMARY KEY,entity_type TEXT,entity_id TEXT,status TEXT,priority INTEGER,
            reason TEXT,stage TEXT,attempts INTEGER,created_at INTEGER,updated_at INTEGER,
            next_attempt_at INTEGER,started_at INTEGER,completed_at INTEGER,last_error TEXT,
            UNIQUE(entity_type,entity_id));
            INSERT INTO enrichment_queue_v1 VALUES(1,'track','old','queued',0,'listening','failed',88,1,2,123,NULL,NULL,'ambiguous');
            INSERT INTO enrichment_queue_v1 VALUES(2,'track','missing','failed',0,'listening','failed',249,1,2,NULL,NULL,NULL,'not found');").unwrap();
        super::super::works::create_schema(&conn).unwrap();
        migrate(&conn).unwrap();
        let counters:(i64,i64,i64)=conn.query_row("SELECT attempts,normal_attempts,agent_attempts FROM enrichment_queue_v1 WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(counters, (88, 12, 0));
        assert_eq!(
            conn.query_row(
                "SELECT status FROM enrichment_queue_v1 WHERE id=2",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "failed_enrichment"
        );
        conn.execute(
            "UPDATE enrichment_queue_v1 SET agent_attempts=1 WHERE id=1",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT agent_attempts FROM enrichment_queue_v1 WHERE id=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM enrichment_attempts_v1 WHERE phase='legacy'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn legacy_model_work_links_are_archived_and_detached_once_but_explicit_links_survive() {
        let (store, temp) = setup();
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute_batch("INSERT INTO works_v1 VALUES('w','Title','[\"Writer\"]',NULL,'song','identity',1);
                INSERT INTO work_resolutions_v1 VALUES('weak','w','created','model guessed','{\"final_response\":\"no matching work\"}',1,2,3,'wikidata_supported_v1');
                INSERT INTO work_resolutions_v1 VALUES('strong','w','linked','explicit relation','{}',1,2,3,'musicbrainz_supported_v1');
                INSERT INTO enrichment_queue_v1(entity_type,entity_id,status) VALUES('work_resolution','weak','completed');").unwrap();
        }
        drop(store);
        let store = SqliteEnrichmentStore::new(
            temp.path().join("enrichment.db"),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        assert!(store
            .get_work_resolution("weak")
            .unwrap()
            .unwrap()
            .work
            .is_none());
        assert!(store
            .get_work_resolution("strong")
            .unwrap()
            .unwrap()
            .work
            .is_some());
        assert_eq!(
            store
                .get_enrichment_queue_item("work_resolution", "weak")
                .unwrap()
                .unwrap()
                .status,
            "failed_enrichment"
        );
        let conn = store.read_conn.lock().unwrap();
        let archived:(String,String,String)=conn.query_row(
            "SELECT work_id,original_reason,evidence_json FROM work_link_quarantine_v1 WHERE track_id='weak'",[],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(archived.0, "w");
        assert_eq!(archived.1, "model guessed");
        assert_eq!(
            serde_json::from_str::<Value>(&archived.2).unwrap()["final_response"],
            "no matching work"
        );
        drop(conn);
        migrate(&store.write_conn.lock().unwrap()).unwrap();
        assert_eq!(
            store
                .read_conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM work_link_quarantine_v1", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
