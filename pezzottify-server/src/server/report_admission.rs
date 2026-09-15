//! Shared admission for modern and legacy reports, before any body buffering.
use super::{http_layers::requests_logging::is_report_path, session::Session, state::ServerState};
use crate::{db_executor::DbPriority, server_store::reports::MAX_BODY_BYTES, user::Permission};
use axum::{
    body::{to_bytes, Body},
    extract::{FromRequestParts, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct UserWindow {
    started: Option<Instant>,
    attempts: usize,
    requests: usize,
    active: usize,
}
#[derive(Default)]
struct Counters {
    users: HashMap<usize, UserWindow>,
    active: usize,
    rejected: [u64; 8],
    expiration_failures: u64,
    last_expiration: Option<i64>,
}
pub(super) struct Admission {
    server: ServerState,
    counters: Mutex<Counters>,
}
struct Slot {
    state: Arc<Admission>,
    user: usize,
}
impl Drop for Slot {
    fn drop(&mut self) {
        if let Ok(mut c) = self.state.counters.lock() {
            c.active = c.active.saturating_sub(1);
            if let Some(u) = c.users.get_mut(&self.user) {
                u.active = u.active.saturating_sub(1);
            }
        }
    }
}
impl Admission {
    pub fn new(server: ServerState) -> Arc<Self> {
        let state = Arc::new(Self {
            server,
            counters: Mutex::new(Counters::default()),
        });
        let weak = Arc::downgrade(&state);
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(60));
            loop {
                timer.tick().await;
                let Some(state) = weak.upgrade() else { break };
                let result = state
                    .server
                    .database
                    .server
                    .run(DbPriority::Interactive, |s| {
                        s.reports()
                            .ok_or_else(|| anyhow::anyhow!("Report repository unavailable"))?
                            .expire()
                            .map_err(Into::into)
                    })
                    .await;
                if let Ok(mut counters) = state.counters.lock() {
                    if result.is_ok() {
                        counters.last_expiration = Some(chrono::Utc::now().timestamp());
                    } else {
                        counters.expiration_failures =
                            counters.expiration_failures.saturating_add(1);
                    }
                };
            }
        });
        state
    }
    pub fn stats(&self) -> serde_json::Value {
        let Ok(c) = self.counters.lock() else {
            return serde_json::json!({"unavailable":true});
        };
        serde_json::json!({"active_requests":c.active,"rejections_since_start":c.rejected,
            "expiration_failures_since_start":c.expiration_failures,"last_successful_expiration":c.last_expiration,
            "rejection_statuses":[400,401,403,409,413,415,429,503],"body_bytes":MAX_BODY_BYTES,
            "body_deadline_seconds":30,"global_slots":8,"user_slots":2,"submission_attempts_per_minute":30})
    }
    fn acquire(self: &Arc<Self>, user: usize, submit: bool) -> Result<Slot, StatusCode> {
        let mut c = self
            .counters
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        reserve_slot(&mut c, user, submit, Instant::now())?;
        Ok(Slot {
            state: self.clone(),
            user,
        })
    }
}
fn reserve_slot(
    c: &mut Counters,
    user: usize,
    submit: bool,
    now: Instant,
) -> Result<(), StatusCode> {
    c.users.retain(|_, u| {
        u.active > 0
            || u.started
                .is_some_and(|t| now.duration_since(t) < Duration::from_secs(60))
    });
    if !c.users.contains_key(&user) && c.users.len() >= 4096 {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let u = c.users.entry(user).or_default();
    if u.started
        .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(60))
    {
        u.started = Some(now);
        u.attempts = 0;
        u.requests = 0;
    }
    u.requests = u.requests.saturating_add(1);
    if submit {
        u.attempts = u.attempts.saturating_add(1);
    }
    if u.requests > 120 || u.attempts > 30 || u.active >= 2 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if c.active >= 8 {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    c.active += 1;
    c.users.get_mut(&user).unwrap().active += 1;
    Ok(())
}
pub(super) fn error(status: StatusCode) -> Response {
    let (code, message) = match status.as_u16() {
        401 => ("unauthenticated", "Sign in to access reports"),
        403 => ("forbidden", "Report permission required"),
        404 => ("not_found", "Report not found"),
        409 => ("conflict", "Report changed or request key was reused"),
        413 => (
            "payload_too_large",
            "Report or attachment exceeds the size limit",
        ),
        415 => ("unsupported_encoding", "Send uncompressed JSON"),
        429 => ("quota_exceeded", "Report limit reached; retry later"),
        503 => ("capacity_exceeded", "Report capacity reached; retry later"),
        500 => ("storage_error", "Report storage failed"),
        _ => (
            "invalid_report",
            "Invalid report request or body read deadline exceeded",
        ),
    };
    let mut response = (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response();
    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
        response
            .headers_mut()
            .insert("retry-after", "3600".parse().unwrap());
    }
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
pub(super) async fn gate(
    State(state): State<Arc<Admission>>,
    request: Request,
    next: Next,
) -> Response {
    if !is_report_path(request.uri().path()) {
        return next.run(request).await;
    }
    let response = admit(state.clone(), request, next).await;
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        if let Some(i) = [400, 401, 403, 409, 413, 415, 429, 503]
            .iter()
            .position(|n| *n == status.as_u16())
        {
            if let Ok(mut c) = state.counters.lock() {
                c.rejected[i] = c.rejected[i].saturating_add(1);
            }
        }
        return error(if status == StatusCode::UNPROCESSABLE_ENTITY {
            StatusCode::BAD_REQUEST
        } else {
            status
        });
    }
    response
}
async fn admit(state: Arc<Admission>, request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let session = match Session::from_request_parts(&mut parts, &state.server).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let path = parts.uri.path();
    let required = if path.starts_with("/v1/admin/") {
        if path.contains("/attachments/")
            || path.ends_with("/events")
            || path.starts_with("/v1/admin/bug-report/")
        {
            Permission::ViewReportDiagnostics
        } else if path.contains("/settings")
            || path.contains("/integrations")
            || path.contains("/deliveries")
        {
            Permission::ManageReportIntegrations
        } else {
            Permission::TriageReports
        }
    } else {
        Permission::ReportBug
    };
    if !session.has_permission(required) {
        return error(StatusCode::FORBIDDEN);
    }
    let submit = parts.method == axum::http::Method::POST
        && (path == "/v1/reports" || path == "/v1/user/bug-report");
    let _slot = match state.acquire(session.user_id, submit) {
        Ok(s) => s,
        Err(s) => return error(s),
    };
    if let Some(id) = path.strip_prefix("/v1/admin/bug-report/") {
        if parts.method == axum::http::Method::GET || parts.method == axum::http::Method::DELETE {
            let id = id.to_string();
            let actor = session.user_id;
            let delete = parts.method == axum::http::Method::DELETE;
            match state
                .server
                .database
                .server
                .run(DbPriority::Interactive, move |s| {
                    Ok(s.reports()
                        .ok_or(crate::server_store::reports::ReportError::Storage)
                        .and_then(|r| r.audit_legacy(&id, actor, delete)))
                })
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return e.into_response(),
                Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
    if parts
        .headers
        .get_all("content-encoding")
        .iter()
        .any(|v| v.as_bytes() != b"identity")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let max = if path == "/v1/user/bug-report" {
        2 * 1024 * 1024
    } else if submit {
        MAX_BODY_BYTES
    } else {
        128 * 1024 // Includes JSON escaping overhead for a 16 KiB internal note.
    };
    let bytes = match read_body(body, max, Duration::from_secs(30)).await {
        Ok(bytes) => bytes,
        Err(status) => return error(status),
    };
    parts.extensions.insert(state);
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}

async fn read_body(
    body: Body,
    max: usize,
    deadline: Duration,
) -> Result<axum::body::Bytes, StatusCode> {
    match tokio::time::timeout(deadline, to_bytes(body, max)).await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Err(StatusCode::PAYLOAD_TOO_LARGE),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_slots_attempts_and_user_windows_are_bounded() {
        let now = Instant::now();
        let mut c = Counters::default();
        for user in 0..4 {
            for _ in 0..2 {
                reserve_slot(&mut c, user, true, now).unwrap();
            }
        }
        assert_eq!(
            reserve_slot(&mut c, 0, true, now),
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
        assert_eq!(
            reserve_slot(&mut c, 4, true, now),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
        assert_eq!(c.active, 8);
        c = Counters::default();
        for _ in 0..30 {
            reserve_slot(&mut c, 1, true, now).unwrap();
            c.active = 0;
            c.users.get_mut(&1).unwrap().active = 0;
        }
        assert_eq!(
            reserve_slot(&mut c, 1, true, now),
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
        reserve_slot(&mut c, 1, true, now + Duration::from_secs(61)).unwrap();
        c = Counters::default();
        for user in 0..4096 {
            reserve_slot(&mut c, user, true, now).unwrap();
            c.active = 0;
            c.users.get_mut(&user).unwrap().active = 0;
        }
        assert_eq!(
            reserve_slot(&mut c, 4096, true, now),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
        reserve_slot(&mut c, 4096, true, now + Duration::from_secs(61)).unwrap();
        assert_eq!(c.users.len(), 1);
    }
    #[tokio::test]
    async fn report_body_size_is_incremental_and_reads_have_deadlines() {
        let bytes = axum::body::Bytes::from_static(b"12345678");
        let stream = futures::stream::iter(vec![
            Ok::<_, std::io::Error>(bytes.clone()),
            Ok(bytes.clone()),
        ]);
        assert_eq!(
            read_body(Body::from_stream(stream), 15, Duration::from_secs(1))
                .await
                .unwrap_err(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        let stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(bytes.clone()), Ok(bytes)]);
        assert_eq!(
            read_body(Body::from_stream(stream), 16, Duration::from_secs(1))
                .await
                .unwrap()
                .len(),
            16
        );
        let pending = futures::stream::pending::<Result<axum::body::Bytes, std::io::Error>>();
        assert_eq!(
            read_body(Body::from_stream(pending), 16, Duration::from_millis(10))
                .await
                .unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
