use super::{reports::*, SqliteServerStore};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

// Caller holds a write transaction so legacy deletion cannot orphan extensions.
pub(super) fn delete_legacy(conn: &Connection, id: &str) -> ReportResult<usize> {
    conn.execute("DELETE FROM report_attachments WHERE report_id=?1", [id])?;
    conn.execute("DELETE FROM report_events WHERE report_id=?1", [id])?;
    conn.execute("DELETE FROM report_metadata WHERE report_id=?1", [id])?;
    conn.execute("UPDATE report_metadata SET duplicate_of=NULL,version=version+1,updated_at=?2 WHERE duplicate_of=?1", params![id,now()])?;
    Ok(conn.execute("DELETE FROM bug_reports WHERE id=?1", [id])?)
}

pub(super) fn index_legacy(conn: &Connection, report: &super::BugReport) -> ReportResult<()> {
    let created = report.created_at.timestamp();
    let retention = super::report_limits::settings(conn)?.retention_days as i64 * 86400;
    conn.execute("INSERT INTO report_metadata(report_id,user_id,kind,category,status,version,request_key,payload_hash,updated_at)
        VALUES (?1,?2,'bug','other','new',1,?1,'legacy',?3)",params![report.id,report.user_id as i64,created])?;
    for (kind, content) in [
        ("technical_logs", report.logs.as_ref()),
        ("legacy_images", report.attachments.as_ref()),
    ] {
        if let Some(content) = content {
            conn.execute("INSERT INTO report_attachments(id,report_id,kind,content,size_bytes,created_at,expires_at,state)
                VALUES (?1,?2,?3,?4,?5,?6,?7,'active')",
                params![uuid::Uuid::new_v4().to_string(),report.id,kind,content,content.len() as i64,created,created+retention])?;
        }
    }
    event(
        conn,
        &report.id,
        report.user_id,
        "submitted",
        serde_json::json!({"legacy":true,"version":1}),
    )
}

pub(super) fn event(
    conn: &Connection,
    id: &str,
    actor: usize,
    kind: &str,
    data: serde_json::Value,
) -> ReportResult<()> {
    super::report_limits::event_capacity(conn, id, data.to_string().len())?;
    conn.execute("INSERT INTO report_events(id,report_id,actor_id,kind,data,created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![uuid::Uuid::new_v4().to_string(),id,actor as i64,kind,data.to_string(),now()])?;
    Ok(())
}

fn attachment_info(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentInfo> {
    Ok(AttachmentInfo {
        id: row.get(0)?,
        kind: row.get(1)?,
        size_bytes: row.get::<_, i64>(2)? as usize,
        created_at: row.get(3)?,
        expires_at: row.get(4)?,
        state: row.get(5)?,
    })
}
fn attachments(conn: &Connection, id: &str) -> ReportResult<Vec<AttachmentInfo>> {
    let mut q = conn.prepare(
        "SELECT id,kind,size_bytes,created_at,expires_at,
        CASE WHEN state='active' AND expires_at<=?2 THEN 'expired' ELSE state END
        FROM report_attachments WHERE report_id=?1 ORDER BY created_at,id",
    )?;
    let rows = q
        .query_map(params![id, now()], attachment_info)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

const REPORT_SELECT: &str = "SELECT m.seq,b.id,b.user_id,b.user_handle,m.kind,m.category,m.status,m.version,
 b.title,b.description,b.client_type,b.client_version,b.device_info,b.created_at,m.updated_at,m.duplicate_of
 FROM bug_reports b JOIN report_metadata m ON m.report_id=b.id";
fn report_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Report> {
    Ok(Report {
        seq: row.get(0)?,
        id: row.get(1)?,
        user_id: row.get::<_, i64>(2)? as usize,
        user_handle: row.get(3)?,
        kind: row.get(4)?,
        category: row.get(5)?,
        status: row.get(6)?,
        version: row.get(7)?,
        title: row.get(8)?,
        description: row.get(9)?,
        client_type: row.get(10)?,
        client_version: row.get(11)?,
        device_info: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        duplicate_of: row.get(15)?,
        attachments: vec![],
    })
}
fn get(conn: &Connection, id: &str, owner: Option<usize>) -> ReportResult<Report> {
    let mut report = conn
        .query_row(
            &format!("{REPORT_SELECT} WHERE b.id=?1 AND (?2 IS NULL OR b.user_id=?2)"),
            params![id, owner.map(|v| v as i64)],
            report_row,
        )
        .optional()?
        .ok_or(ReportError::NotFound)?;
    report.attachments = attachments(conn, id)?;
    Ok(report)
}

impl ReportRepository for SqliteServerStore {
    fn audit_legacy(&self, id: &str, actor: usize, delete: bool) -> ReportResult<()> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        get(&tx, id, None)?;
        super::report_limits::expire(&tx, now())?;
        event(
            &tx,
            if delete { "administration" } else { id },
            actor,
            if delete {
                "legacy_delete_requested"
            } else {
                "legacy_diagnostics_read"
            },
            serde_json::json!({"report_id":id}),
        )?;
        tx.commit()?;
        Ok(())
    }
    fn settings(&self) -> ReportResult<ReportSettings> {
        let c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        super::report_limits::settings(&c)
    }
    fn stats(&self) -> ReportResult<ReportStats> {
        let c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        super::report_limits::stats(&c)
    }
    fn expire(&self) -> ReportResult<usize> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        let n = super::report_limits::expire(&tx, now())?;
        tx.commit()?;
        Ok(n)
    }
    fn save_settings(&self, actor: usize, input: ReportSettings) -> ReportResult<ReportSettings> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        let result = super::report_limits::save_settings(&tx, actor, input)?;
        tx.commit()?;
        Ok(result)
    }
    fn create(
        &self,
        user_id: usize,
        user_handle: &str,
        mut input: NewReport,
    ) -> ReportResult<ReportReceipt> {
        input.validate()?;
        input.attachments.sort_by(|a, b| a.kind.cmp(&b.kind));
        let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&input)?));
        let mut conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing:Option<(String,String,i64)>=tx.query_row(
            "SELECT report_id,payload_hash,version FROM report_metadata WHERE user_id=?1 AND request_key=?2",
            params![user_id as i64,input.client_request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((id, previous, version)) = existing {
            if previous != hash {
                return Err(ReportError::Conflict);
            }
            return Ok(ReportReceipt {
                id,
                version,
                replayed: true,
            });
        }
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = now();
        let metadata = input.description.len()
            + input.title.as_ref().map_or(0, String::len)
            + input.client_type.len()
            + input.client_version.as_ref().map_or(0, String::len)
            + input.device_info.as_ref().map_or(0, String::len)
            + user_handle.len();
        let limits = super::report_limits::reserve(
            &tx,
            user_id,
            metadata,
            input.attachments.iter().map(|a| a.content.len()).sum(),
            timestamp,
        )?;
        tx.execute("INSERT INTO bug_reports(id,user_id,user_handle,title,description,client_type,client_version,device_info,created_at)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![id,user_id as i64,user_handle,input.title.as_deref().unwrap_or(""),input.description,
                input.client_type,input.client_version,input.device_info,chrono::Utc::now().to_rfc3339()])?;
        tx.execute("INSERT INTO report_metadata(report_id,user_id,kind,category,status,version,request_key,payload_hash,updated_at)
            VALUES (?1,?2,?3,?4,'new',1,?5,?6,?7)",
            params![id,user_id as i64,input.kind,input.category,input.client_request_id,hash,timestamp])?;
        for attachment in input.attachments {
            tx.execute("INSERT INTO report_attachments(id,report_id,kind,content,size_bytes,created_at,expires_at,state)
                VALUES (?1,?2,?3,?4,?5,?6,?7,'active')",
                params![uuid::Uuid::new_v4().to_string(),id,attachment.kind,attachment.content,
                    attachment.content.len() as i64,timestamp,timestamp+(limits.retention_days*86400) as i64])?;
        }
        event(
            &tx,
            &id,
            user_id,
            "submitted",
            serde_json::json!({"version":1,"kind":input.kind,"category":input.category,"status":"new"}),
        )?;
        tx.commit()?;
        Ok(ReportReceipt {
            id,
            version: 1,
            replayed: false,
        })
    }

    fn get(&self, id: &str, owner: Option<usize>) -> ReportResult<Report> {
        let conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        get(&conn, id, owner)
    }

    fn list(&self, owner: Option<usize>, filter: ReportFilter) -> ReportResult<ReportPage> {
        let limit = filter.limit.unwrap_or(25);
        if !(1..=100).contains(&limit) || filter.before.is_some_and(|v| v < 1) {
            return Err(ReportError::Invalid("Invalid pagination".into()));
        }
        let conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let mut q=conn.prepare(&format!("{REPORT_SELECT}
            WHERE (?1 IS NULL OR b.user_id=?1) AND (?2 IS NULL OR m.seq<?2)
            AND (?3 IS NULL OR m.kind=?3) AND (?4 IS NULL OR m.category=?4)
            AND (?5 IS NULL OR m.status=?5) AND (?6 IS NULL OR b.client_type=?6)
            AND (?7 IS NULL OR b.client_version=?7) AND (?8 IS NULL OR b.user_id=?8)
            AND (?9 IS NULL OR EXISTS(SELECT 1 FROM report_attachments a WHERE a.report_id=b.id AND a.content IS NOT NULL AND a.expires_at>?10)=?9)
            ORDER BY m.seq DESC LIMIT ?11"))?;
        let mut items = q
            .query_map(
                params![
                    owner.map(|v| v as i64),
                    filter.before,
                    filter.kind,
                    filter.category,
                    filter.status,
                    filter.client_type,
                    filter.client_version,
                    filter.user_id.map(|v| v as i64),
                    filter.has_diagnostics,
                    now(),
                    (limit + 1) as i64
                ],
                report_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit;
        items.truncate(limit);
        for report in &mut items {
            // Summary responses omit potentially large description/device text.
            report.description.clear();
            report.device_info = None;
            report.attachments = attachments(&conn, &report.id)?;
        }
        let next_cursor = if has_more {
            items.last().map(|r| r.seq)
        } else {
            None
        };
        Ok(ReportPage { items, next_cursor })
    }

    fn update(&self, id: &str, actor: usize, input: ReportUpdate) -> ReportResult<Report> {
        if input.status.as_ref().is_some_and(|s| {
            !["new", "investigating", "planned", "resolved", "closed"].contains(&s.as_str())
        }) || input
            .note
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 16 * 1024)
            || input.duplicate_of.as_ref().is_some_and(|s| s.len() > 128)
            || (input.status.is_none() && input.note.is_none() && input.duplicate_of.is_none())
        {
            return Err(ReportError::Invalid("Invalid report update".into()));
        }
        let mut conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let previous = get(&tx, id, None)?;
        if previous.version != input.expected_version {
            return Err(ReportError::Conflict);
        }
        let duplicate = input
            .duplicate_of
            .clone()
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .unwrap_or(previous.duplicate_of.clone());
        if let Some(target) = &duplicate {
            let mut cursor = Some(target.clone());
            let mut visited = std::collections::HashSet::new();
            while let Some(target) = cursor {
                if target == id || !visited.insert(target.clone()) || visited.len() > 100 {
                    return Err(ReportError::Invalid(
                        "Duplicate link would create a cycle or excessive chain".into(),
                    ));
                }
                cursor = get(&tx, &target, None)?.duplicate_of;
            }
        }
        let status = input.status.unwrap_or(previous.status);
        tx.execute("UPDATE report_metadata SET status=?1,version=version+1,updated_at=?2,duplicate_of=?3 WHERE report_id=?4",
            params![status,now(),duplicate,id])?;
        event(
            &tx,
            id,
            actor,
            "updated",
            serde_json::json!({"version":input.expected_version+1,"status":status,"duplicate_of":duplicate}),
        )?;
        if let Some(note) = input.note {
            event(&tx, id, actor, "note", serde_json::json!({"text":note}))?;
        }
        let result = get(&tx, id, None)?;
        tx.commit()?;
        Ok(result)
    }

    fn events(&self, id: &str, before: Option<i64>, limit: usize) -> ReportResult<EventPage> {
        if !(1..=100).contains(&limit) {
            return Err(ReportError::Invalid("Invalid pagination".into()));
        }
        let conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        if !["settings", "administration"].contains(&id) {
            get(&conn, id, None)?;
        }
        let mut q = conn.prepare(
            "SELECT seq,id,actor_id,kind,data,created_at FROM report_events
            WHERE report_id=?1 AND (?2 IS NULL OR seq<?2) ORDER BY seq DESC LIMIT ?3",
        )?;
        let mut items = q
            .query_map(params![id, before, (limit + 1) as i64], |r| {
                Ok(ReportEvent {
                    seq: r.get(0)?,
                    id: r.get(1)?,
                    actor_id: r.get::<_, i64>(2)? as usize,
                    kind: r.get(3)?,
                    data: serde_json::from_str(&r.get::<_, String>(4)?)
                        .unwrap_or(serde_json::Value::Null),
                    created_at: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit;
        items.truncate(limit);
        let next_cursor = if has_more {
            items.last().map(|e| e.seq)
        } else {
            None
        };
        Ok(EventPage { items, next_cursor })
    }

    fn attachment(
        &self,
        id: &str,
        attachment: &str,
        owner: Option<usize>,
        actor: usize,
    ) -> ReportResult<AttachmentContent> {
        let mut conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = conn.transaction()?;
        let report = get(&tx, id, owner)?;
        let info = report
            .attachments
            .into_iter()
            .find(|a| a.id == attachment && a.state == "active")
            .ok_or(ReportError::NotFound)?;
        let content = tx
            .query_row(
                "SELECT content FROM report_attachments WHERE id=?1 AND content IS NOT NULL",
                [attachment],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .ok_or(ReportError::NotFound)?;
        event(
            &tx,
            id,
            actor,
            "attachment_read",
            serde_json::json!({"attachment_id":attachment}),
        )?;
        tx.commit()?;
        Ok(AttachmentContent {
            attachment: info,
            content,
        })
    }

    fn delete_attachment(
        &self,
        id: &str,
        attachment: &str,
        owner: Option<usize>,
        actor: usize,
    ) -> ReportResult<()> {
        let mut conn = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = conn.transaction()?;
        let report = get(&tx, id, owner)?;
        if !report.attachments.iter().any(|a| a.id == attachment) {
            return Err(ReportError::NotFound);
        }
        let changed=tx.execute("UPDATE report_attachments SET content=NULL,state='deleted' WHERE id=?1 AND state!='deleted'",[attachment])?;
        if changed > 0 {
            // Clear legacy copies as well, where present.
            let kind = report
                .attachments
                .iter()
                .find(|a| a.id == attachment)
                .unwrap()
                .kind
                .as_str();
            if kind == "technical_logs" {
                tx.execute("UPDATE bug_reports SET logs=NULL WHERE id=?1", [id])?;
            }
            if kind == "legacy_images" {
                tx.execute("UPDATE bug_reports SET attachments=NULL WHERE id=?1", [id])?;
            }
            event(
                &tx,
                id,
                actor,
                "attachment_deleted",
                serde_json::json!({"attachment_id":attachment}),
            )?;
            tx.execute(
                "UPDATE report_metadata SET version=version+1,updated_at=?1 WHERE report_id=?2",
                params![now(), id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::DbRegistry;
    fn setup() -> (tempfile::TempDir, SqliteServerStore) {
        let dir = tempfile::tempdir().unwrap();
        let store =
            SqliteServerStore::new(dir.path().join("server.db"), &DbRegistry::new()).unwrap();
        (dir, store)
    }
    fn input() -> NewReport {
        NewReport {
            client_request_id: uuid::Uuid::new_v4().to_string(),
            kind: "bug".into(),
            category: "assistant".into(),
            title: None,
            description: "Ciao 🌍".into(),
            client_type: "web".into(),
            client_version: None,
            device_info: None,
            attachments: vec![NewAttachment {
                kind: "technical_logs".into(),
                content: "hello".into(),
                consent: true,
            }],
        }
    }
    #[test]
    fn legacy_client_labels_remain_supported_without_relaxing_modern_validation() {
        let mut request = input();
        request.client_type = "rust-integration".into();
        assert!(request.validate_legacy().is_ok());
        assert!(matches!(request.validate(), Err(ReportError::Invalid(_))));
        request.client_type = "x".repeat(4097);
        assert!(matches!(
            request.validate_legacy(),
            Err(ReportError::TooLarge)
        ));
        request.client_type = "android".into();
        request.attachments[0].consent = false;
        assert!(matches!(
            request.validate_legacy(),
            Err(ReportError::Invalid(_))
        ));
    }
    #[test]
    fn concurrent_quota_and_replays_are_atomic() {
        let (_dir, s) = setup();
        let mut settings = s.settings().unwrap();
        settings.hourly_reports = 1;
        s.save_settings(2, settings).unwrap();
        let request = input();
        std::thread::scope(|scope| {
            let results: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| s.create(1, "a", request.clone())))
                .collect();
            let receipts: Vec<_> = results
                .into_iter()
                .map(|j| j.join().unwrap().unwrap())
                .collect();
            assert_eq!(receipts.iter().filter(|r| !r.replayed).count(), 1);
        });
        std::thread::scope(|scope| {
            let results: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| s.create(1, "a", input())))
                .collect();
            for j in results {
                assert!(matches!(j.join().unwrap(), Err(ReportError::Quota)));
            }
        });
        assert_eq!(s.stats().unwrap().reports, 1);
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 5);
    }

    #[test]
    fn user_quota_precedes_global_capacity_and_rejection_does_not_insert() {
        let (_dir, s) = setup();
        let mut settings = s.settings().unwrap();
        settings.hourly_reports = 1;
        settings.total_reports = 1;
        s.save_settings(2, settings).unwrap();
        s.create(1, "a", input()).unwrap();

        assert!(matches!(s.create(1, "a", input()), Err(ReportError::Quota)));
        assert!(matches!(
            s.create(2, "b", input()),
            Err(ReportError::Capacity)
        ));
        assert_eq!(s.stats().unwrap().reports, 1);
    }

    #[test]
    fn report_hour_window_excludes_exact_cutoff_and_includes_next_second() {
        let (_dir, s) = setup();
        let id = s.create(1, "a", input()).unwrap().id;
        let mut settings = s.settings().unwrap();
        settings.hourly_reports = 1;
        s.save_settings(2, settings).unwrap();
        let probe_now = now();
        for (seconds_after_cutoff, expected_quota) in [(0, false), (1, true)] {
            let created =
                chrono::DateTime::from_timestamp(probe_now - 3600 + seconds_after_cutoff, 0)
                    .unwrap()
                    .to_rfc3339();
            let mut conn = s.conn.lock().unwrap();
            conn.execute(
                "UPDATE bug_reports SET created_at=?1 WHERE id=?2",
                params![created, id],
            )
            .unwrap();
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            let result = crate::server_store::report_limits::reserve(&tx, 1, 0, 0, probe_now);
            assert_eq!(matches!(result, Err(ReportError::Quota)), expected_quota);
            tx.rollback().unwrap();
        }
    }
    #[test]
    fn retention_preserves_metadata_and_accounting_uses_bytes() {
        let (_dir, s) = setup();
        let mut req = input();
        req.attachments[0].content = "🌍".into();
        let id = s.create(1, "a", req).unwrap().id;
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 4);
        let attachment = s.get(&id, Some(1)).unwrap().attachments[0].id.clone();
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE report_attachments SET expires_at=0", [])
            .unwrap();
        assert_eq!(s.expire().unwrap(), 1);
        assert_eq!(s.expire().unwrap(), 0);
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 0);
        assert_eq!(s.get(&id, Some(1)).unwrap().description, "Ciao 🌍");
        assert!(matches!(
            s.attachment(&id, &attachment, Some(1), 1),
            Err(ReportError::NotFound)
        ));
    }
    #[test]
    fn diagnostic_capacity_rejects_without_partial_report_and_settings_conflict() {
        let (_dir, s) = setup();
        let mut limits = s.settings().unwrap();
        limits.diagnostic_bytes = 4;
        s.save_settings(2, limits.clone()).unwrap();
        assert!(matches!(
            s.save_settings(2, limits),
            Err(ReportError::Conflict)
        ));
        assert!(matches!(
            s.create(1, "a", input()),
            Err(ReportError::Capacity)
        ));
        assert_eq!(s.stats().unwrap().reports, 0);
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 0);
    }
    #[test]
    fn audit_capacity_reserves_room_for_owner_deletion() {
        let (_dir, s) = setup();
        let id = s.create(1, "a", input()).unwrap().id;
        let attachment = s.get(&id, Some(1)).unwrap().attachments[0].id.clone();
        s.conn.lock().unwrap().execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1998) INSERT INTO report_events(id,report_id,actor_id,kind,data,created_at) SELECT 'filler-'||x,?1,1,'test','{}',0 FROM n",[&id]).unwrap();
        assert!(matches!(
            s.attachment(&id, &attachment, Some(1), 1),
            Err(ReportError::Capacity)
        ));
        s.delete_attachment(&id, &attachment, Some(1), 1).unwrap();
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 0);
    }
    #[test]
    fn legacy_delete_removes_extension_data() {
        use crate::server_store::ServerStore;
        let (_dir, s) = setup();
        let report = s.create(1, "alice", input()).unwrap();
        assert!(s.delete_bug_report(&report.id).unwrap());
        assert!(matches!(
            s.get(&report.id, Some(1)),
            Err(ReportError::NotFound)
        ));
        let conn = s.conn.lock().unwrap();
        for table in ["report_metadata", "report_attachments", "report_events"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }
    #[test]
    fn create_replay_conflict_and_owner_isolation() {
        let (_dir, s) = setup();
        let mut req = input();
        let result = s.create(1, "alice", req.clone()).unwrap();
        assert!(s.create(1, "alice", req.clone()).unwrap().replayed);
        assert!(matches!(
            s.get(&result.id, Some(2)),
            Err(ReportError::NotFound)
        ));
        req.description = "changed".into();
        assert!(matches!(
            s.create(1, "alice", req),
            Err(ReportError::Conflict)
        ));
        let report = s.get(&result.id, Some(1)).unwrap();
        assert_eq!(report.description, "Ciao 🌍");
        assert!(matches!(
            s.attachment(&result.id, &report.attachments[0].id, Some(2), 2),
            Err(ReportError::NotFound)
        ));
        s.attachment(&result.id, &report.attachments[0].id, Some(1), 1)
            .unwrap();
        s.delete_attachment(&result.id, &report.attachments[0].id, Some(1), 1)
            .unwrap();
        assert_eq!(
            s.get(&result.id, Some(1)).unwrap().attachments[0].state,
            "deleted"
        );
        assert_eq!(s.events(&result.id, None, 25).unwrap().items.len(), 3);
    }
    #[test]
    fn optimistic_updates_and_duplicate_cycles() {
        let (_dir, s) = setup();
        let a = s.create(1, "a", input()).unwrap();
        let b = s.create(1, "a", input()).unwrap();
        s.update(
            &a.id,
            9,
            ReportUpdate {
                expected_version: 1,
                status: Some("planned".into()),
                note: Some("review".into()),
                duplicate_of: Some(b.id.clone()),
            },
        )
        .unwrap();
        assert!(matches!(
            s.update(
                &a.id,
                9,
                ReportUpdate {
                    expected_version: 1,
                    status: Some("new".into()),
                    note: None,
                    duplicate_of: None
                }
            ),
            Err(ReportError::Conflict)
        ));
        assert!(s
            .update(
                &b.id,
                9,
                ReportUpdate {
                    expected_version: 1,
                    status: None,
                    note: None,
                    duplicate_of: Some(a.id)
                }
            )
            .is_err());
    }
    #[test]
    fn cursor_is_stable_across_new_submissions() {
        let (_dir, s) = setup();
        for _ in 0..3 {
            s.create(1, "a", input()).unwrap();
        }
        let first = s
            .list(
                Some(1),
                ReportFilter {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        s.create(1, "a", input()).unwrap();
        let second = s
            .list(
                Some(1),
                ReportFilter {
                    before: first.next_cursor,
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.items[0].seq < first.items[1].seq);
        assert!(s
            .list(Some(2), ReportFilter::default())
            .unwrap()
            .items
            .is_empty());
    }
    #[test]
    fn fixture_and_utf8_limits() {
        validate_diagnostics(include_str!(
            "../../../docs/fixtures/assistant-diagnostics-v1.json"
        ))
        .unwrap();
        let mut req = input();
        req.attachments[0].content = "🌍".repeat(MAX_LOG_BYTES / 4 + 1);
        assert!(matches!(req.validate(), Err(ReportError::TooLarge)));
    }
    #[test]
    fn migrates_legacy_reports_without_losing_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.db");
        let conn = Connection::open(&path).unwrap();
        super::super::SERVER_VERSIONED_SCHEMAS[6]
            .create(&conn)
            .unwrap();
        conn.execute("INSERT INTO bug_reports(id,user_id,user_handle,title,description,client_type,logs,created_at)
            VALUES ('legacy',1,'alice','title','description','android','🌍','2099-01-01T00:00:00Z')",[]).unwrap();
        drop(conn);
        let store = SqliteServerStore::new(path, &DbRegistry::new()).unwrap();
        let r = store.get("legacy", Some(1)).unwrap();
        assert_eq!(r.kind, "bug");
        assert_eq!(r.attachments[0].size_bytes, 4);
        assert_eq!(
            store
                .attachment("legacy", &r.attachments[0].id, Some(1), 1)
                .unwrap()
                .content,
            "🌍"
        );
    }
}
