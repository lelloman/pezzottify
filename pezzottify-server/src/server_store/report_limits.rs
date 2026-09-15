use super::{report_repository::event, reports::*};
use rusqlite::{params, Connection, OptionalExtension};

pub(super) fn settings(c: &Connection) -> ReportResult<ReportSettings> {
    let json: Option<String> = c
        .query_row(
            "SELECT value FROM server_state WHERE key='report_settings'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let value = json
        .map(|j| serde_json::from_str::<ReportSettings>(&j))
        .transpose()?
        .unwrap_or_default();
    value.validate()?;
    Ok(value)
}
pub(super) fn stats(c: &Connection) -> ReportResult<ReportStats> {
    let (reports, metadata_bytes): (usize,usize) = c.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(id || COALESCE(title,'') || description || client_type || COALESCE(client_version,'') || COALESCE(device_info,'') || user_handle || created_at AS BLOB))),0) FROM bug_reports", [], |r| Ok((r.get(0)?,r.get(1)?)))?;
    let extension_bytes:usize=c.query_row("SELECT COALESCE((SELECT SUM(length(CAST(report_id || kind || category || status || request_key || payload_hash || COALESCE(duplicate_of,'') AS BLOB))) FROM report_metadata),0) + COALESCE((SELECT SUM(length(CAST(id || report_id || kind || state AS BLOB))) FROM report_attachments),0)",[],|r|r.get(0))?;
    let (events, event_bytes): (usize, usize) = c.query_row(
        "SELECT COUNT(*),COALESCE(SUM(length(CAST(id || report_id || kind || data AS BLOB))),0) FROM report_events",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let diagnostic_bytes = c.query_row(
        "SELECT COALESCE(SUM(length(CAST(content AS BLOB))),0) FROM report_attachments",
        [],
        |r| r.get(0),
    )?;
    Ok(ReportStats {
        reports,
        metadata_bytes: metadata_bytes + event_bytes + extension_bytes,
        diagnostic_bytes,
        events,
        settings: settings(c)?,
    })
}

// Always called inside the same write transaction as insert/reservation.
pub(super) fn reserve(
    c: &Connection,
    user: usize,
    metadata: usize,
    bytes: usize,
    now: i64,
) -> ReportResult<ReportSettings> {
    expire(c, now)?;
    let s = stats(c)?;
    let limits = s.settings;
    let (total,hour,day): (usize,usize,usize) = c.query_row("SELECT COUNT(*),COALESCE(SUM(unixepoch(created_at)>?2-3600),0),COALESCE(SUM(unixepoch(created_at)>?2-86400),0) FROM bug_reports WHERE user_id=?1", params![user as i64,now], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let daily_bytes: usize = c.query_row("SELECT COALESCE(SUM(a.size_bytes),0) FROM report_attachments a JOIN report_metadata m ON a.report_id=m.report_id WHERE m.user_id=?1 AND a.created_at>?2-86400", params![user as i64,now], |r| r.get(0))?;
    if total >= limits.user_reports
        || hour >= limits.hourly_reports
        || day >= limits.daily_reports
        || daily_bytes.saturating_add(bytes) > limits.daily_attachment_bytes
    {
        return Err(ReportError::Quota);
    }
    // Reserve room for the submitted event, not just the free text.
    if s.reports >= limits.total_reports
        || s.metadata_bytes
            .saturating_add(metadata)
            .saturating_add(1024)
            > limits.metadata_bytes
        || s.diagnostic_bytes.saturating_add(bytes) > limits.diagnostic_bytes
    {
        return Err(ReportError::Capacity);
    }
    Ok(limits)
}
pub(super) fn event_capacity(c: &Connection, id: &str, bytes: usize) -> ReportResult<()> {
    let s = stats(c)?;
    let count: usize = c.query_row(
        "SELECT COUNT(*) FROM report_events WHERE report_id=?1",
        [id],
        |r| r.get(0),
    )?;
    // Reserve one small audit event per live attachment so capacity cannot prevent
    // its owner from deleting it. Deletion changes state before appending its event.
    let (active,local):(usize,usize)=c.query_row("SELECT COUNT(*),COALESCE(SUM(report_id=?1),0) FROM report_attachments WHERE state='active' AND content IS NOT NULL",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if s.events.saturating_add(active) >= 200_000
        || count.saturating_add(local) >= 2000
        || s.metadata_bytes
            .saturating_add(active.saturating_mul(512))
            .saturating_add(bytes)
            .saturating_add(256)
            > s.settings.metadata_bytes
    {
        return Err(ReportError::Capacity);
    }
    Ok(())
}
pub(super) fn expire(c: &Connection, now: i64) -> ReportResult<usize> {
    let changed = c.execute("UPDATE report_attachments SET content=NULL,state='expired' WHERE state='active' AND expires_at<=?1", [now])?;
    // Eliminate the migration-era copies too; legacy reads hydrate from typed storage.
    c.execute("UPDATE bug_reports SET logs=NULL,attachments=NULL WHERE logs IS NOT NULL OR attachments IS NOT NULL", [])?;
    Ok(changed)
}
pub(super) fn save_settings(
    c: &Connection,
    actor: usize,
    mut input: ReportSettings,
) -> ReportResult<ReportSettings> {
    input.validate()?;
    if settings(c)?.version != input.version {
        return Err(ReportError::Conflict);
    }
    input.version += 1;
    event(
        c,
        "settings",
        actor,
        "settings_updated",
        serde_json::to_value(&input)?,
    )?;
    c.execute("INSERT INTO server_state(key,value,updated_at) VALUES ('report_settings',?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at", params![serde_json::to_string(&input)?,chrono::Utc::now().to_rfc3339()])?;
    // Shorter retention applies immediately. Increasing never resurrects/extends consented data.
    c.execute("UPDATE report_attachments SET expires_at=MIN(expires_at,created_at+?1) WHERE state='active'", [(input.retention_days*86400) as i64])?;
    expire(c, chrono::Utc::now().timestamp())?;
    let used = stats(c)?;
    let active: usize = c.query_row(
        "SELECT COUNT(*) FROM report_attachments WHERE state='active' AND content IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    if used
        .metadata_bytes
        .saturating_add(active.saturating_mul(512))
        > input.metadata_bytes
    {
        return Err(ReportError::Invalid(
            "Metadata limit is below retained data and deletion audit reservations".into(),
        ));
    }
    Ok(input)
}
