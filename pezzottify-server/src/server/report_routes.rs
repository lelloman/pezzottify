//! Feedback endpoints backed by the versioned report repository.
use super::{
    session::Session,
    state::{DatabaseHandles, ServerState},
};
use crate::{db_executor::DbPriority, server_store::reports::*, user::Permission};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;

impl IntoResponse for ReportError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Invalid(_) => (StatusCode::BAD_REQUEST, "invalid_report"),
            Self::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict => (StatusCode::CONFLICT, "conflict"),
            Self::Quota => (StatusCode::TOO_MANY_REQUESTS, "quota_exceeded"),
            Self::Capacity => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exceeded"),
            Self::Storage => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
        };
        let mut response = (
            status,
            Json(serde_json::json!({"error":{"code":code,"message":self.to_string()}})),
        )
            .into_response();
        if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("3600"));
        }
        response
    }
}

fn permission(session: &Session, admin: bool) -> Result<(), Response> {
    let permission = if admin {
        Permission::ServerAdmin
    } else {
        Permission::ReportBug
    };
    if session.has_permission(permission) {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN,Json(serde_json::json!({"error":{"code":"forbidden","message":"Report permission required"}}))).into_response())
    }
}

async fn run<T: Send + 'static>(
    db: DatabaseHandles,
    f: impl FnOnce(&dyn ReportRepository) -> ReportResult<T> + Send + 'static,
) -> ReportResult<T> {
    db.server
        .run(DbPriority::Interactive, move |s| {
            Ok(s.reports().ok_or(ReportError::Storage).and_then(f))
        })
        .await
        .map_err(|_| ReportError::Storage)?
}
fn output<T: Serialize>(result: ReportResult<T>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn submit(
    session: Session,
    State(db): State<DatabaseHandles>,
    Json(input): Json<NewReport>,
) -> Response {
    if let Err(r) = permission(&session, false) {
        return r;
    }
    let uid = session.user_id;
    let handle = match db
        .user_manager
        .run(DbPriority::Interactive, move |m| m.get_user_handle(uid))
        .await
    {
        Ok(Some(handle)) => handle,
        _ => return ReportError::Storage.into_response(),
    };
    match run(db, move |s| s.create(uid, &handle, input)).await {
        Ok(receipt) => (
            if receipt.replayed {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            Json(receipt),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}
async fn owner_list(
    session: Session,
    State(db): State<DatabaseHandles>,
    Query(filter): Query<ReportFilter>,
) -> Response {
    if let Err(r) = permission(&session, false) {
        return r;
    }
    output(run(db, move |s| s.list(Some(session.user_id), filter)).await)
}
async fn admin_list(
    session: Session,
    State(db): State<DatabaseHandles>,
    Query(filter): Query<ReportFilter>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(run(db, move |s| s.list(None, filter)).await)
}
async fn owner_get(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = permission(&session, false) {
        return r;
    }
    output(run(db, move |s| s.get(&id, Some(session.user_id))).await)
}
async fn admin_get(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(run(db, move |s| s.get(&id, None)).await)
}
async fn update(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
    Json(input): Json<ReportUpdate>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(run(db, move |s| s.update(&id, session.user_id, input)).await)
}
async fn events(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
    Query(query): Query<ReportFilter>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(
        run(db, move |s| {
            s.events(&id, query.before, query.limit.unwrap_or(25))
        })
        .await,
    )
}
async fn owner_attachment(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path((id, attachment)): Path<(String, String)>,
) -> Response {
    if let Err(r) = permission(&session, false) {
        return r;
    }
    output(
        run(db, move |s| {
            s.attachment(&id, &attachment, Some(session.user_id), session.user_id)
        })
        .await,
    )
}
async fn admin_attachment(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path((id, attachment)): Path<(String, String)>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(
        run(db, move |s| {
            s.attachment(&id, &attachment, None, session.user_id)
        })
        .await,
    )
}
async fn owner_delete_attachment(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path((id, attachment)): Path<(String, String)>,
) -> Response {
    if let Err(r) = permission(&session, false) {
        return r;
    }
    output(
        run(db, move |s| {
            s.delete_attachment(&id, &attachment, Some(session.user_id), session.user_id)
        })
        .await,
    )
}
async fn admin_delete_attachment(
    session: Session,
    State(db): State<DatabaseHandles>,
    Path((id, attachment)): Path<(String, String)>,
) -> Response {
    if let Err(r) = permission(&session, true) {
        return r;
    }
    output(
        run(db, move |s| {
            s.delete_attachment(&id, &attachment, None, session.user_id)
        })
        .await,
    )
}

pub fn routes(state: ServerState) -> Router {
    Router::new()
        .route("/v1/reports", post(submit).get(owner_list))
        .route("/v1/reports/{id}", get(owner_get))
        .route(
            "/v1/reports/{id}/attachments/{attachment}",
            get(owner_attachment).delete(owner_delete_attachment),
        )
        .route("/v1/admin/reports", get(admin_list))
        .route("/v1/admin/reports/{id}", get(admin_get).patch(update))
        .route("/v1/admin/reports/{id}/events", get(events))
        .route(
            "/v1/admin/reports/{id}/attachments/{attachment}",
            get(admin_attachment).delete(admin_delete_attachment),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}
