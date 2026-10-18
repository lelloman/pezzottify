//! Durable, bounded report delivery state. Every mutation uses a write transaction.
use super::{reports::*, SqliteServerStore};
use crate::{
    sqlite_column,
    sqlite_persistence::{Column, SqlType, Table},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const MAX_INTEGRATIONS: usize = 10;
pub const MAX_DELIVERIES: usize = 10_000;
pub const MAX_ATTEMPTS: i64 = 5;
pub const MAX_MANUAL_RETRIES: i64 = 5;
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
pub const LEASE_SECONDS: i64 = 60;
pub(super) const INTEGRATIONS: Table = Table {
    name: "report_integrations",
    columns: &[
        sqlite_column!("seq", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true),
        sqlite_column!("url", &SqlType::Text, non_null = true),
        sqlite_column!("secret", &SqlType::Text, non_null = true),
        sqlite_column!("enabled", &SqlType::Integer, non_null = true),
        sqlite_column!("version", &SqlType::Integer, non_null = true),
    ],
    indices: &[("idx_report_integrations_id", "id")],
    unique_constraints: &[&["id"]],
};
pub(super) const DELIVERIES: Table = Table {
    name: "report_deliveries",
    columns: &[
        sqlite_column!("seq", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true),
        sqlite_column!("event_id", &SqlType::Text, non_null = true),
        sqlite_column!("integration_id", &SqlType::Text, non_null = true),
        sqlite_column!("body", &SqlType::Text, non_null = true),
        sqlite_column!("state", &SqlType::Text, non_null = true),
        sqlite_column!("attempts", &SqlType::Integer, non_null = true),
        sqlite_column!("manual_retries", &SqlType::Integer, non_null = true),
        sqlite_column!("next_at", &SqlType::Integer, non_null = true),
        sqlite_column!("lease_token", &SqlType::Text, non_null = true),
        sqlite_column!("version", &SqlType::Integer, non_null = true),
        sqlite_column!("outcome", &SqlType::Text, non_null = true),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[
        ("idx_report_deliveries_due", "state,next_at"),
        ("idx_report_deliveries_id", "id"),
    ],
    unique_constraints: &[&["id"]],
};
// Separate bounded storage keeps monitoring and general report capacity from
// disabling safety controls. Only fixed kinds and bounded resource IDs enter it.
const AUDIT_PARTITION_LIMIT: usize = 1_000;
pub(super) const AUTOMATION_AUDIT: Table = Table {
    name: "report_automation_audit",
    columns: super::report_schema::REPORT_EVENTS.columns,
    indices: &[(
        "idx_report_automation_audit_partition",
        "report_id, seq DESC",
    )],
    unique_constraints: &[&["id"]],
};
pub(super) fn migrate(c: &Connection) -> anyhow::Result<()> {
    INTEGRATIONS.create(c)?;
    DELIVERIES.create(c)?;
    AUTOMATION_AUDIT.create(c)?;
    Ok(())
}

#[derive(Serialize)]
pub struct Integration {
    pub id: String,
    pub url: String,
    pub enabled: bool,
    pub version: i64,
}
#[derive(Serialize)]
pub struct CreatedIntegration {
    pub integration: Integration,
    /// Base64-encoded HMAC key; returned once, never included in list responses.
    pub secret: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewIntegration {
    pub url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationUpdate {
    pub expected_version: i64,
    pub enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRetry {
    pub expected_version: i64,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryFilter {
    pub before: Option<i64>,
    pub limit: Option<usize>,
    pub state: Option<String>,
    pub integration_id: Option<String>,
}
#[derive(Serialize)]
pub struct Delivery {
    pub seq: i64,
    pub id: String,
    pub event_id: String,
    pub integration_id: String,
    pub state: String,
    pub attempts: i64,
    pub manual_retries: i64,
    pub next_at: i64,
    pub version: i64,
    pub outcome: String,
}
#[derive(Serialize)]
pub struct DeliveryPage {
    pub items: Vec<Delivery>,
    pub next_cursor: Option<i64>,
}
#[derive(Serialize)]
pub struct DeliveryStats {
    pub pending: usize,
    pub leased: usize,
    pub succeeded: usize,
    pub failed: usize,
}
// No Debug/Serialize: URLs, payloads and secrets must not escape into worker logs.
pub struct ClaimedDelivery {
    pub id: String,
    pub event_id: String,
    pub url: String,
    pub secret: String,
    pub body: String,
    pub token: String,
}
pub trait AutomationRepository: Send + Sync {
    fn integrations(&self, actor: usize) -> ReportResult<Vec<Integration>>;
    fn create_integration(
        &self,
        actor: usize,
        input: NewIntegration,
    ) -> ReportResult<CreatedIntegration>;
    fn update_integration(
        &self,
        actor: usize,
        id: &str,
        input: IntegrationUpdate,
    ) -> ReportResult<Integration>;
    fn deliveries(&self, actor: usize, filter: DeliveryFilter) -> ReportResult<DeliveryPage>;
    fn retry_delivery(&self, actor: usize, id: &str, input: DeliveryRetry) -> ReportResult<()>;
    fn delivery_stats(&self) -> ReportResult<DeliveryStats>;
    fn claim_delivery(&self, now: i64) -> ReportResult<Option<ClaimedDelivery>>;
    fn finish_delivery(
        &self,
        id: &str,
        token: &str,
        outcome: &str,
        success: bool,
        retry_after: i64,
        now: i64,
    ) -> ReportResult<()>;
}

pub fn validate_destination(value: &str) -> ReportResult<reqwest::Url> {
    let invalid = || {
        ReportError::Invalid(
            "Destination must be a public HTTPS URL without credentials or fragment".into(),
        )
    };
    if value.len() > 2048 {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    if let Some(host) = url.host_str() {
        if let Ok(ip) = host.trim_matches(['[', ']']).parse() {
            if !public_address(ip) {
                return Err(invalid());
            }
        }
    }
    Ok(url)
}
/// Conservative global-unicast allowlist; special-purpose ranges fail closed.
pub fn public_address(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168
                        || (b == 0 && c == 0)
                        || (b == 0 && c == 2)
                        || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub(super) fn enqueue(
    c: &Connection,
    report: &str,
    event_id: &str,
    kind: &str,
    now: i64,
) -> ReportResult<()> {
    // Only metadata changes are eligible. Reads, diagnostics, notes and control-plane
    // audit events cannot trigger external sharing or consume the report deletion reserve.
    if !["submitted", "updated"].contains(&kind) {
        return Ok(());
    }
    let destinations = {
        let mut q = c.prepare("SELECT id FROM report_integrations WHERE enabled=1 ORDER BY seq")?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if destinations.is_empty() {
        return Ok(());
    }
    // Retain terminal inspection records for 30 days. Pending/leased work is never evicted.
    c.execute(
        "DELETE FROM report_deliveries WHERE state IN ('succeeded','failed') AND next_at<?1",
        [now - 30 * 86400],
    )?;
    let count: usize = c.query_row("SELECT COUNT(*) FROM report_deliveries", [], |r| r.get(0))?;
    if count + destinations.len() > MAX_DELIVERIES {
        return Err(ReportError::Capacity);
    }
    let body = c.query_row("SELECT kind,category,status,version FROM report_metadata WHERE report_id=?1", [report], |r| {
        Ok(serde_json::json!({"schema_version":1,"event_id":event_id,"event_type":kind,"created_at":now,
            "report":{"id":report,"kind":r.get::<_,String>(0)?,"category":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"version":r.get::<_,i64>(3)?}}).to_string())
    })?;
    if body.len() > MAX_PAYLOAD_BYTES {
        return Err(ReportError::TooLarge);
    }
    for integration in destinations {
        c.execute("INSERT INTO report_deliveries(id,event_id,integration_id,body,state,attempts,manual_retries,next_at,lease_token,version,outcome,created_at) VALUES (?1,?2,?3,?4,'pending',0,0,?5,'',1,'',?5)",
            params![uuid::Uuid::new_v4().to_string(),event_id,integration,body,now])?;
    }
    Ok(())
}
fn integration(c: &Connection, id: &str) -> ReportResult<Integration> {
    c.query_row(
        "SELECT id,url,enabled,version FROM report_integrations WHERE id=?1",
        [id],
        integration_row,
    )
    .optional()?
    .ok_or(ReportError::NotFound)
}
fn integration_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Integration> {
    Ok(Integration {
        id: r.get(0)?,
        url: r.get(1)?,
        enabled: r.get(2)?,
        version: r.get(3)?,
    })
}
fn audit(c: &Connection, actor: usize, kind: &str, id: &str) -> ReportResult<()> {
    let partition = if matches!(kind, "integrations_listed" | "deliveries_listed") {
        "inspection"
    } else {
        "control"
    };
    let data = serde_json::json!({"resource_id":id}).to_string();
    if id.len() > 128 || kind.len() > 64 || data.len() > 256 {
        return Err(ReportError::Invalid(
            "Invalid automation audit record".into(),
        ));
    }
    let now = chrono::Utc::now().timestamp();
    // Pruning and insertion commit atomically with the operation. Retain at most
    // 30 days and 1,000 records per partition, evicting oldest records on overflow.
    c.execute(
        "DELETE FROM report_automation_audit WHERE created_at<?1",
        [now - 30 * 86400],
    )?;
    c.execute(
        "DELETE FROM report_automation_audit WHERE report_id=?1 AND seq NOT IN
        (SELECT seq FROM report_automation_audit WHERE report_id=?1 ORDER BY seq DESC LIMIT ?2)",
        params![partition, (AUDIT_PARTITION_LIMIT - 1) as i64],
    )?;
    c.execute(
        "INSERT INTO report_automation_audit(id,report_id,actor_id,kind,data,created_at)
        VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            uuid::Uuid::new_v4().to_string(),
            partition,
            actor as i64,
            kind,
            data,
            now
        ],
    )?;
    Ok(())
}
fn delivery_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Delivery> {
    Ok(Delivery {
        seq: r.get(0)?,
        id: r.get(1)?,
        event_id: r.get(2)?,
        integration_id: r.get(3)?,
        state: r.get(4)?,
        attempts: r.get(5)?,
        manual_retries: r.get(6)?,
        next_at: r.get(7)?,
        version: r.get(8)?,
        outcome: r.get(9)?,
    })
}
impl AutomationRepository for SqliteServerStore {
    fn integrations(&self, actor: usize) -> ReportResult<Vec<Integration>> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        audit(&tx, actor, "integrations_listed", "integrations")?;
        let rows = {
            let mut q =
                tx.prepare("SELECT id,url,enabled,version FROM report_integrations ORDER BY seq")?;
            let rows = q
                .query_map([], integration_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        tx.commit()?;
        Ok(rows)
    }
    fn create_integration(
        &self,
        actor: usize,
        input: NewIntegration,
    ) -> ReportResult<CreatedIntegration> {
        let url = validate_destination(&input.url)?.to_string();
        let secret = STANDARD.encode(rand::random::<[u8; 32]>());
        let id = uuid::Uuid::new_v4().to_string();
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count: usize =
            tx.query_row("SELECT COUNT(*) FROM report_integrations", [], |r| r.get(0))?;
        if count >= MAX_INTEGRATIONS {
            return Err(ReportError::Capacity);
        }
        tx.execute(
            "INSERT INTO report_integrations(id,url,secret,enabled,version) VALUES (?1,?2,?3,0,1)",
            params![id, url, secret],
        )?;
        audit(&tx, actor, "integration_created", &id)?;
        let result = integration(&tx, &id)?;
        tx.commit()?;
        Ok(CreatedIntegration {
            integration: result,
            secret,
        })
    }
    fn update_integration(
        &self,
        actor: usize,
        id: &str,
        input: IntegrationUpdate,
    ) -> ReportResult<Integration> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        let old = integration(&tx, id)?;
        if old.version != input.expected_version {
            return Err(ReportError::Conflict);
        }
        tx.execute(
            "UPDATE report_integrations SET enabled=?1,version=version+1 WHERE id=?2",
            params![input.enabled, id],
        )?;
        audit(
            &tx,
            actor,
            if input.enabled {
                "integration_enabled"
            } else {
                "integration_disabled"
            },
            id,
        )?;
        let result = integration(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    fn deliveries(&self, actor: usize, filter: DeliveryFilter) -> ReportResult<DeliveryPage> {
        let limit = filter.limit.unwrap_or(25);
        if !(1..=100).contains(&limit)
            || filter.before.is_some_and(|v| v < 1)
            || filter.state.as_ref().is_some_and(|s| {
                !["pending", "leased", "succeeded", "failed"].contains(&s.as_str())
            })
            || filter
                .integration_id
                .as_ref()
                .is_some_and(|s| s.len() > 128)
        {
            return Err(ReportError::Invalid("Invalid delivery filter".into()));
        }
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        audit(&tx, actor, "deliveries_listed", "deliveries")?;
        let mut rows = {
            let mut q=tx.prepare("SELECT seq,id,event_id,integration_id,state,attempts,manual_retries,next_at,version,outcome FROM report_deliveries WHERE (?1 IS NULL OR seq<?1) AND (?2 IS NULL OR state=?2) AND (?3 IS NULL OR integration_id=?3) ORDER BY seq DESC LIMIT ?4")?;
            let rows = q
                .query_map(
                    params![
                        filter.before,
                        filter.state,
                        filter.integration_id,
                        (limit + 1) as i64
                    ],
                    delivery_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next_cursor = if more {
            rows.last().map(|d| d.seq)
        } else {
            None
        };
        tx.commit()?;
        Ok(DeliveryPage {
            items: rows,
            next_cursor,
        })
    }
    fn retry_delivery(&self, actor: usize, id: &str, input: DeliveryRetry) -> ReportResult<()> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction()?;
        let (version, state, manual): (i64, String, i64) = tx
            .query_row(
                "SELECT version,state,manual_retries FROM report_deliveries WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or(ReportError::NotFound)?;
        if version != input.expected_version || state != "failed" || manual >= MAX_MANUAL_RETRIES {
            return Err(ReportError::Conflict);
        }
        tx.execute("UPDATE report_deliveries SET state='pending',manual_retries=manual_retries+1,next_at=?1,version=version+1 WHERE id=?2",params![chrono::Utc::now().timestamp(),id])?;
        audit(&tx, actor, "delivery_retried", id)?;
        tx.commit()?;
        Ok(())
    }
    fn delivery_stats(&self) -> ReportResult<DeliveryStats> {
        let c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        Ok(c.query_row("SELECT COALESCE(SUM(state='pending'),0),COALESCE(SUM(state='leased'),0),COALESCE(SUM(state='succeeded'),0),COALESCE(SUM(state='failed'),0) FROM report_deliveries",[],|r|Ok(DeliveryStats{pending:r.get(0)?,leased:r.get(1)?,succeeded:r.get(2)?,failed:r.get(3)?}))?)
    }
    fn claim_delivery(&self, now: i64) -> ReportResult<Option<ClaimedDelivery>> {
        let mut c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // A claim consumes an attempt, even if the process dies before recording its outcome.
        tx.execute("UPDATE report_deliveries SET state='failed',outcome='lease_expired',version=version+1 WHERE state='leased' AND next_at<=?1 AND attempts>=?2+manual_retries",params![now,MAX_ATTEMPTS])?;
        let row:Option<ClaimedDelivery>=tx.query_row("SELECT d.id,d.event_id,i.url,i.secret,d.body FROM report_deliveries d JOIN report_integrations i ON i.id=d.integration_id WHERE i.enabled=1 AND ((d.state='pending' AND d.next_at<=?1) OR (d.state='leased' AND d.next_at<=?1)) AND d.attempts<?2+d.manual_retries ORDER BY d.next_at,d.seq LIMIT 1",params![now,MAX_ATTEMPTS],|r|Ok(ClaimedDelivery{id:r.get(0)?,event_id:r.get(1)?,url:r.get(2)?,secret:r.get(3)?,body:r.get(4)?,token:uuid::Uuid::new_v4().to_string()})).optional()?;
        if let Some(d) = &row {
            tx.execute("UPDATE report_deliveries SET state='leased',attempts=attempts+1,next_at=?1,lease_token=?2,version=version+1 WHERE id=?3",params![now+LEASE_SECONDS,d.token,d.id])?;
        }
        tx.commit()?;
        Ok(row)
    }
    fn finish_delivery(
        &self,
        id: &str,
        token: &str,
        outcome: &str,
        success: bool,
        retry_after: i64,
        now: i64,
    ) -> ReportResult<()> {
        if ![
            "delivered",
            "http_error",
            "transport_error",
            "response_too_large",
            "destination_rejected",
            "deadline",
        ]
        .contains(&outcome)
            && !outcome
                .strip_prefix("http_")
                .and_then(|s| s.parse::<u16>().ok())
                .is_some_and(|s| (100..=599).contains(&s))
        {
            return Err(ReportError::Invalid("Invalid delivery outcome".into()));
        }
        let c = self.conn.lock().map_err(|_| ReportError::Storage)?;
        // CAS rejects late workers after another process recovered the lease. Tokens are
        // random per claim; a stale completion cannot overwrite newer state.
        c.execute("UPDATE report_deliveries SET state=CASE WHEN ?1 THEN 'succeeded' WHEN attempts>=?2+manual_retries THEN 'failed' ELSE 'pending' END,outcome=?3,next_at=?4+CASE WHEN ?1 OR attempts>=?2+manual_retries THEN 0 ELSE MAX(MIN(?5,3600),MIN(3600,30*(1 << MIN(attempts-1,7)))) END,version=version+1,lease_token='' WHERE id=?6 AND lease_token=?7 AND state='leased'",params![success,MAX_ATTEMPTS,outcome,now,retry_after.clamp(0,3600),id,token])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backup::DbRegistry,
        server_store::{reports::ReportRepository, ServerStore},
    };
    fn setup() -> (tempfile::TempDir, SqliteServerStore) {
        let dir = tempfile::tempdir().unwrap();
        let s = SqliteServerStore::new(dir.path().join("server.db"), &DbRegistry::new()).unwrap();
        (dir, s)
    }
    fn request() -> NewReport {
        NewReport {
            client_request_id: uuid::Uuid::new_v4().to_string(),
            kind: "feature".into(),
            category: "assistant".into(),
            title: Some("PRIVATE title".into()),
            description: "PRIVATE description".into(),
            client_type: "web".into(),
            client_version: None,
            device_info: None,
            attachments: vec![NewAttachment {
                kind: "technical_logs".into(),
                content: "PRIVATE diagnostic".into(),
                consent: true,
            }],
        }
    }
    fn enabled(s: &SqliteServerStore) -> CreatedIntegration {
        let d = s
            .create_integration(
                9,
                NewIntegration {
                    url: "https://example.invalid/webhook".into(),
                },
            )
            .unwrap();
        s.update_integration(
            9,
            &d.integration.id,
            IntegrationUpdate {
                expected_version: 1,
                enabled: true,
            },
        )
        .unwrap();
        d
    }
    fn delivery(s: &SqliteServerStore) -> Delivery {
        s.deliveries(9, DeliveryFilter::default())
            .unwrap()
            .items
            .remove(0)
    }
    #[test]
    fn disabled_by_default_secret_once_cap_and_audit() {
        let (_dir, s) = setup();
        let d = s
            .create_integration(
                9,
                NewIntegration {
                    url: "https://example.invalid/".into(),
                },
            )
            .unwrap();
        assert!(!d.integration.enabled);
        assert_eq!(STANDARD.decode(&d.secret).unwrap().len(), 32);
        let list = serde_json::to_string(&s.integrations(9).unwrap()).unwrap();
        assert!(!list.contains("secret"));
        assert!(!list.contains(&d.secret));
        s.create(1, "PRIVATE user", request()).unwrap();
        assert!(s
            .claim_delivery(i64::MAX - LEASE_SECONDS)
            .unwrap()
            .is_none());
        assert!(matches!(
            s.update_integration(
                9,
                &d.integration.id,
                IntegrationUpdate {
                    expected_version: 2,
                    enabled: true
                }
            ),
            Err(ReportError::Conflict)
        ));
        for _ in 1..MAX_INTEGRATIONS {
            s.create_integration(
                9,
                NewIntegration {
                    url: "https://example.invalid/".into(),
                },
            )
            .unwrap();
        }
        assert!(matches!(
            s.create_integration(
                9,
                NewIntegration {
                    url: "https://example.invalid/".into()
                }
            ),
            Err(ReportError::Capacity)
        ));
        let events = s.events("automation", None, 100).unwrap();
        assert_eq!(
            events
                .items
                .iter()
                .filter(|e| e.kind == "integration_created")
                .count(),
            MAX_INTEGRATIONS
        );
        assert!(events
            .items
            .iter()
            .all(|e| e.actor_id == 9 && !e.data.to_string().contains(&d.secret)));
    }
    #[test]
    fn audit_saturation_never_blocks_disable_and_retention_recovers() {
        let (_dir, s) = setup();
        let integration = enabled(&s);
        s.create(1, "a", request()).unwrap();
        // Reproduce the reviewer's polling sequence beyond the old shared budget.
        for _ in 0..2_001 {
            s.deliveries(9, DeliveryFilter::default()).unwrap();
        }
        {
            let c = s.conn.lock().unwrap();
            let inspections: usize = c
                .query_row(
                    "SELECT COUNT(*) FROM report_automation_audit WHERE report_id='inspection'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(inspections, AUDIT_PARTITION_LIMIT);
            // Exhaust the general global count and administration resource budget.
            c.execute(
                "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<200000)
                INSERT INTO report_events(id,report_id,actor_id,kind,data,created_at)
                SELECT 'filler-'||x,'administration',1,'test','{}',0 FROM n",
                [],
            )
            .unwrap();
            // Exhaust the configured metadata ceiling as well.
            let limits = ReportSettings {
                metadata_bytes: 1,
                ..ReportSettings::default()
            };
            c.execute("INSERT INTO server_state(key,value,updated_at) VALUES ('report_settings',?1,'test')",
                [serde_json::to_string(&limits).unwrap()]).unwrap();
            // Fill the independent control partition, including expired records.
            c.execute(
                "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000)
                INSERT INTO report_automation_audit(id,report_id,actor_id,kind,data,created_at)
                SELECT 'control-'||x,'control',1,'test','{}',?1 FROM n",
                [chrono::Utc::now().timestamp()],
            )
            .unwrap();
            c.execute(
                "INSERT INTO report_automation_audit(id,report_id,actor_id,kind,data,created_at)
                VALUES ('expired','inspection',1,'test','{}',0)",
                [],
            )
            .unwrap();
        }
        // General audit storage really is full; its errors must not affect controls.
        assert!(matches!(
            super::super::report_limits::event_capacity(
                &s.conn.lock().unwrap(),
                "administration",
                2
            ),
            Err(ReportError::Capacity)
        ));
        let disabled = s
            .update_integration(
                9,
                &integration.integration.id,
                IntegrationUpdate {
                    expected_version: 2,
                    enabled: false,
                },
            )
            .unwrap();
        assert!(!disabled.enabled);
        assert!(s
            .claim_delivery(chrono::Utc::now().timestamp())
            .unwrap()
            .is_none());
        let events = s.events("automation", None, 1).unwrap();
        assert_eq!(events.items[0].kind, "integration_disabled");
        assert_eq!(events.items[0].actor_id, 9);
        assert_eq!(
            events.items[0].data["resource_id"],
            integration.integration.id
        );
        let c = s.conn.lock().unwrap();
        let (controls, expired): (usize, usize) = c
            .query_row(
                "SELECT SUM(report_id='control'),SUM(id='expired') FROM report_automation_audit",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(controls, AUDIT_PARTITION_LIMIT);
        assert_eq!(expired, 0);
        assert!(events.next_cursor.is_some());
        drop(c);
        assert!(s.events("automation", events.next_cursor, 1).is_ok());
    }
    #[test]
    fn failed_automation_audit_rolls_back_control_and_pruning() {
        let (_dir, s) = setup();
        let integration = enabled(&s);
        {
            let c = s.conn.lock().unwrap();
            c.execute("UPDATE report_automation_audit SET created_at=0", [])
                .unwrap();
            c.execute_batch(
                "CREATE TRIGGER fail_automation_audit BEFORE INSERT ON report_automation_audit
                BEGIN SELECT RAISE(ABORT, 'test audit failure'); END;",
            )
            .unwrap();
        }
        assert!(s
            .update_integration(
                9,
                &integration.integration.id,
                IntegrationUpdate {
                    expected_version: 2,
                    enabled: false
                }
            )
            .is_err());
        {
            let c = s.conn.lock().unwrap();
            let current = super::integration(&c, &integration.integration.id).unwrap();
            assert!(current.enabled);
            assert_eq!(current.version, 2);
            let retained: usize = c
                .query_row(
                    "SELECT COUNT(*) FROM report_automation_audit WHERE created_at=0",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(retained, 2);
            c.execute_batch("DROP TRIGGER fail_automation_audit")
                .unwrap();
        }
        // A transient audit failure does not cache a failed state or poison the store.
        assert!(
            !s.update_integration(
                9,
                &integration.integration.id,
                IntegrationUpdate {
                    expected_version: 2,
                    enabled: false
                }
            )
            .unwrap()
            .enabled
        );
    }
    #[test]
    fn durable_atomic_private_and_idempotent_fanout() {
        let (dir, s) = setup();
        enabled(&s);
        enabled(&s);
        let req = request();
        let id = s.create(1, "PRIVATE user", req.clone()).unwrap().id;
        s.create(1, "PRIVATE user", req).unwrap();
        let events = s.events(&id, None, 25).unwrap();
        assert_eq!(events.items.len(), 1);
        assert_eq!(s.delivery_stats().unwrap().pending, 2);
        let now = chrono::Utc::now().timestamp();
        let a = s.claim_delivery(now).unwrap().unwrap();
        let b = s.claim_delivery(now).unwrap().unwrap();
        assert_eq!(a.event_id, events.items[0].id);
        assert_eq!(a.event_id, b.event_id);
        assert_ne!(a.id, b.id);
        assert_eq!(a.body, b.body);
        assert!(!a.body.contains("PRIVATE"));
        let body: serde_json::Value = serde_json::from_str(&a.body).unwrap();
        assert_eq!(body["schema_version"], 1);
        assert_eq!(body["report"]["id"], id);
        assert_eq!(body["report"].as_object().unwrap().len(), 5);
        s.update(
            &id,
            9,
            ReportUpdate {
                expected_version: 1,
                status: Some("planned".into()),
                note: Some("PRIVATE note".into()),
                duplicate_of: None,
            },
        )
        .unwrap();
        assert_eq!(s.delivery_stats().unwrap().pending, 2); // note itself adds no delivery
        drop(s);
        let reopened =
            SqliteServerStore::new(dir.path().join("server.db"), &DbRegistry::new()).unwrap();
        assert_eq!(reopened.delivery_stats().unwrap().leased, 2);
        assert_eq!(reopened.delivery_stats().unwrap().pending, 2);
    }
    #[test]
    fn outbox_capacity_rolls_back_submission_and_update_and_legacy() {
        let (_dir, s) = setup();
        enabled(&s);
        let id = s.create(1, "a", request()).unwrap().id;
        s.conn.lock().unwrap().execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<9999) INSERT INTO report_deliveries(id,event_id,integration_id,body,state,attempts,manual_retries,next_at,lease_token,version,outcome,created_at) SELECT 'filler-'||x,'e','i','{}','pending',0,0,0,'',1,'',0 FROM n",[]).unwrap();
        assert!(matches!(
            s.create(2, "b", request()),
            Err(ReportError::Capacity)
        ));
        assert_eq!(s.stats().unwrap().reports, 1);
        assert!(matches!(
            s.update(
                &id,
                9,
                ReportUpdate {
                    expected_version: 1,
                    status: Some("closed".into()),
                    note: None,
                    duplicate_of: None
                }
            ),
            Err(ReportError::Capacity)
        ));
        assert_eq!(s.get(&id, None).unwrap().version, 1);
        let legacy = super::super::BugReport {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: 2,
            user_handle: "b".into(),
            title: None,
            description: "legacy".into(),
            client_type: "android".into(),
            client_version: None,
            device_info: None,
            logs: None,
            attachments: None,
            created_at: chrono::Utc::now(),
        };
        assert!(s.insert_bug_report(&legacy).is_err());
        assert_eq!(s.stats().unwrap().reports, 1);
        // Sensitive owner operations stay available even when the outbox is full.
        let attachment = s.get(&id, Some(1)).unwrap().attachments[0].id.clone();
        s.delete_attachment(&id, &attachment, Some(1), 1).unwrap();
        assert_eq!(s.stats().unwrap().diagnostic_bytes, 0);
    }
    #[test]
    fn leases_survive_crashes_and_stale_completions_and_disable() {
        let (_dir, s) = setup();
        let integration = enabled(&s);
        s.create(1, "a", request()).unwrap();
        let now = chrono::Utc::now().timestamp();
        let a = s.claim_delivery(now).unwrap().unwrap();
        assert!(s.claim_delivery(now).unwrap().is_none());
        s.update_integration(
            9,
            &integration.integration.id,
            IntegrationUpdate {
                expected_version: 2,
                enabled: false,
            },
        )
        .unwrap();
        assert!(s.claim_delivery(now + LEASE_SECONDS).unwrap().is_none());
        s.update_integration(
            9,
            &integration.integration.id,
            IntegrationUpdate {
                expected_version: 3,
                enabled: true,
            },
        )
        .unwrap();
        let b = s.claim_delivery(now + LEASE_SECONDS).unwrap().unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.body, b.body);
        assert_ne!(a.token, b.token);
        s.finish_delivery(&a.id, &a.token, "delivered", true, 0, now + LEASE_SECONDS)
            .unwrap();
        assert_eq!(delivery(&s).state, "leased");
        s.finish_delivery(&b.id, &b.token, "delivered", true, 0, now + LEASE_SECONDS)
            .unwrap();
        assert_eq!(delivery(&s).state, "succeeded");
        assert_eq!(delivery(&s).attempts, 2);
    }
    #[test]
    fn backoff_retry_after_terminal_and_manual_retries_are_bounded() {
        let (_dir, s) = setup();
        enabled(&s);
        s.create(1, "a", request()).unwrap();
        let mut now = chrono::Utc::now().timestamp();
        for attempt in 1..=MAX_ATTEMPTS {
            let d = s.claim_delivery(now).unwrap().unwrap();
            s.finish_delivery(
                &d.id,
                &d.token,
                "http_error",
                false,
                if attempt == 1 { i64::MAX } else { 0 },
                now,
            )
            .unwrap();
            let record = delivery(&s);
            assert_eq!(record.attempts, attempt);
            if attempt < MAX_ATTEMPTS {
                let expected = if attempt == 1 {
                    3600
                } else {
                    30 * (1 << (attempt - 1))
                };
                assert_eq!(record.next_at - now, expected);
                assert!(s.claim_delivery(record.next_at - 1).unwrap().is_none());
                now = record.next_at;
            } else {
                assert_eq!(record.state, "failed");
            }
        }
        assert!(s.claim_delivery(now + 86400).unwrap().is_none());
        for _ in 0..MAX_MANUAL_RETRIES {
            let record = delivery(&s);
            assert!(matches!(
                s.retry_delivery(
                    9,
                    &record.id,
                    DeliveryRetry {
                        expected_version: record.version - 1
                    }
                ),
                Err(ReportError::Conflict)
            ));
            s.retry_delivery(
                9,
                &record.id,
                DeliveryRetry {
                    expected_version: record.version,
                },
            )
            .unwrap();
            let d = s.claim_delivery(now + 86400).unwrap().unwrap();
            s.finish_delivery(&d.id, &d.token, "transport_error", false, 0, now + 86400)
                .unwrap();
            assert_eq!(delivery(&s).state, "failed");
        }
        let record = delivery(&s);
        assert_eq!(record.attempts, 10);
        assert!(matches!(
            s.retry_delivery(
                9,
                &record.id,
                DeliveryRetry {
                    expected_version: record.version
                }
            ),
            Err(ReportError::Conflict)
        ));
    }
    #[test]
    fn crashed_final_attempt_becomes_visible_terminal_failure() {
        let (_dir, s) = setup();
        enabled(&s);
        s.create(1, "a", request()).unwrap();
        let mut now = chrono::Utc::now().timestamp();
        for _ in 0..MAX_ATTEMPTS {
            assert!(s.claim_delivery(now).unwrap().is_some());
            now += LEASE_SECONDS;
        }
        assert!(s.claim_delivery(now).unwrap().is_none());
        let d = delivery(&s);
        assert_eq!(d.state, "failed");
        assert_eq!(d.outcome, "lease_expired");
    }
    #[test]
    fn destinations_reject_special_addresses_and_url_bypasses() {
        for value in [
            "http://example.invalid",
            "https://user:password@example.invalid",
            "https://example.invalid/#fragment",
            "https://127.1/",
            "https://2130706433/",
            "https://0x7f000001/",
            "https://[::ffff:8.8.8.8]/",
            "https://[2001:db8::1]/",
        ] {
            assert!(validate_destination(value).is_err(), "{value}");
        }
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.1.1",
            "192.0.0.1",
            "192.0.2.1",
            "192.88.99.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "2001::1",
            "2001:20::1",
            "2001:db8::1",
            "2002:808:808::1",
            "3fff::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public_address(ip.parse().unwrap()), "{ip}");
        }
        assert!(validate_destination("https://example.invalid/path").is_ok());
    }
    #[test]
    fn v8_migration_preserves_reports_and_configures_no_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.db");
        let c = Connection::open(&path).unwrap();
        super::super::SERVER_VERSIONED_SCHEMAS[7]
            .create(&c)
            .unwrap();
        c.execute("INSERT INTO bug_reports(id,user_id,user_handle,title,description,client_type,created_at) VALUES ('old',1,'a','','report','web','2026-01-01')",[]).unwrap();
        drop(c);
        let s = SqliteServerStore::new(path, &DbRegistry::new()).unwrap();
        assert!(s.get_bug_report("old").unwrap().is_some());
        assert!(s.integrations(9).unwrap().is_empty());
    }
}
