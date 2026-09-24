use super::{report_repository::event, reports::*};
use rusqlite::{params, Connection, OptionalExtension};
use simple_server::rate_limit::{
    evaluate_limits, evaluate_rolling_windows, LimitCheck, RollingWindow, RollingWindowError,
    RollingWindowStore,
};

struct ReportWindowStore<'a> {
    connection: &'a Connection,
    user: i64,
    now_micros: i128,
}

impl RollingWindowStore for ReportWindowStore<'_> {
    type Error = ReportError;

    fn now_micros(&mut self) -> ReportResult<i128> {
        Ok(self.now_micros)
    }

    fn count_after(&mut self, window_index: usize, cutoff_micros: i128) -> ReportResult<u64> {
        let cutoff_seconds =
            i64::try_from(cutoff_micros / 1_000_000).map_err(|_| ReportError::Storage)?;
        let used: usize = if window_index == 2 {
            self.connection.query_row(
                "SELECT COALESCE(SUM(a.size_bytes),0) FROM report_attachments a JOIN report_metadata m ON a.report_id=m.report_id WHERE m.user_id=?1 AND a.created_at>?2",
                params![self.user, cutoff_seconds],
                |row| row.get(0),
            )?
        } else {
            self.connection.query_row(
                "SELECT COUNT(*) FROM bug_reports WHERE user_id=?1 AND unixepoch(created_at)>?2",
                params![self.user, cutoff_seconds],
                |row| row.get(0),
            )?
        };
        u64::try_from(used).map_err(|_| ReportError::Storage)
    }
}

#[derive(Clone, Copy)]
enum ReportLimit {
    Quota,
    Capacity,
}

fn limit_error(limit: ReportLimit) -> ReportError {
    match limit {
        ReportLimit::Quota => ReportError::Quota,
        ReportLimit::Capacity => ReportError::Capacity,
    }
}

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
    let total: usize = c.query_row(
        "SELECT COUNT(*) FROM bug_reports WHERE user_id=?1",
        [user as i64],
        |row| row.get(0),
    )?;
    let windows = evaluate_rolling_windows(
        &[
            RollingWindow {
                window_micros: 3_600_000_000,
                limit: limits.hourly_reports as u64,
            },
            RollingWindow {
                window_micros: 86_400_000_000,
                limit: limits.daily_reports as u64,
            },
            RollingWindow {
                window_micros: 86_400_000_000,
                limit: limits.daily_attachment_bytes as u64,
            },
        ],
        u64::MAX,
        &mut ReportWindowStore {
            connection: c,
            user: user as i64,
            now_micros: i128::from(now) * 1_000_000,
        },
    )
    .map_err(|error| match error {
        RollingWindowError::Backend(error) => error,
        RollingWindowError::TimestampRange { .. } => ReportError::Storage,
    })?;
    // The ordered policy keeps user quota errors ahead of global capacity
    // errors. SQLite still owns the snapshot and the surrounding write transaction.
    evaluate_limits(&[
        LimitCheck::below(
            ReportLimit::Quota,
            total as i128,
            limits.user_reports as i128,
        ),
        LimitCheck::below(
            ReportLimit::Quota,
            windows.windows[0].used as i128,
            limits.hourly_reports as i128,
        ),
        LimitCheck::below(
            ReportLimit::Quota,
            windows.windows[1].used as i128,
            limits.daily_reports as i128,
        ),
        LimitCheck::projected(
            ReportLimit::Quota,
            windows.windows[2].used as usize,
            bytes,
            0,
            limits.daily_attachment_bytes,
        ),
        LimitCheck::below(
            ReportLimit::Capacity,
            s.reports as i128,
            limits.total_reports as i128,
        ),
        // Reserve room for the submitted event, not just the free text.
        LimitCheck::projected(
            ReportLimit::Capacity,
            s.metadata_bytes,
            metadata,
            1024,
            limits.metadata_bytes,
        ),
        LimitCheck::projected(
            ReportLimit::Capacity,
            s.diagnostic_bytes,
            bytes,
            0,
            limits.diagnostic_bytes,
        ),
    ])
    .map_err(limit_error)?;
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
    evaluate_limits(&[
        LimitCheck::projected(ReportLimit::Capacity, s.events, active, 0, 199_999),
        LimitCheck::projected(ReportLimit::Capacity, count, local, 0, 1999),
        LimitCheck::projected(
            ReportLimit::Capacity,
            s.metadata_bytes,
            bytes,
            active.saturating_mul(512).saturating_add(256),
            s.settings.metadata_bytes,
        ),
    ])
    .map_err(limit_error)?;
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
    if evaluate_limits(&[LimitCheck::projected(
        ReportLimit::Capacity,
        used.metadata_bytes,
        0,
        active.saturating_mul(512),
        input.metadata_bytes,
    )])
    .is_err()
    {
        return Err(ReportError::Invalid(
            "Metadata limit is below retained data and deletion audit reservations".into(),
        ));
    }
    Ok(input)
}
