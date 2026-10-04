//! UnifiedPush registration routes. See docs/unifiedpush.md.

use serde::{Deserialize, Serialize};
use simple_server::extract::Extract;
use simple_server::web::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};

use super::api_error::ApiError;
use super::session::Session;
use super::state::ServerState;
use crate::db_executor::DbPriority;
use crate::push::{validate_endpoint, validate_keys, PushService};
use std::sync::Arc;

#[derive(Debug, Serialize)]
struct VapidResponse {
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct RegistrationBody {
    endpoint: String,
    p256dh: String,
    auth: String,
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UnregisterBody {
    endpoint: String,
}

pub(crate) fn push_routes() -> Router<ServerState> {
    Router::new().route("/vapid", get(get_vapid)).route(
        "/registrations",
        put(put_registration).delete(delete_registration),
    )
}

fn service(state: &ServerState) -> Result<&Arc<PushService>, ApiError> {
    state.push.as_ref().ok_or_else(ApiError::push_disabled)
}

async fn get_vapid(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
) -> Response {
    match service(&state) {
        Ok(push) => Json(VapidResponse {
            public_key: push.public_key().to_string(),
        })
        .into_response(),
        Err(error) => error.into_response(),
    }
}

async fn put_registration(
    Extract(session): Extract<Session>,
    State(state): State<ServerState>,
    Json(body): Json<RegistrationBody>,
) -> Response {
    let push = match service(&state) {
        Ok(push) => push,
        Err(error) => return error.into_response(),
    };
    if let Err(error) = validate_endpoint(&body.endpoint, push.settings().allow_insecure_endpoints)
        .and_then(|_| validate_keys(&body.p256dh, &body.auth))
    {
        return ApiError::bad_request("invalid_push_registration", error.to_string())
            .into_response();
    }
    if body.device_id.as_ref().is_some_and(|id| id.len() > 128) {
        return ApiError::bad_request("invalid_push_registration", "device_id too long")
            .into_response();
    }
    let user_id = session.user_id;
    match state
        .database
        .user_store
        .run(DbPriority::Interactive, move |store| {
            store.upsert_push_registration(
                user_id,
                &body.endpoint,
                &body.p256dh,
                &body.auth,
                body.device_id.as_deref(),
            )
        })
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => ApiError::user_database(error).into_response(),
    }
}

async fn delete_registration(
    Extract(session): Extract<Session>,
    State(state): State<ServerState>,
    Json(body): Json<UnregisterBody>,
) -> Response {
    if let Err(error) = service(&state) {
        return error.into_response();
    }
    let user_id = session.user_id;
    match state
        .database
        .user_store
        .run(DbPriority::Interactive, move |store| {
            store.delete_push_registration(user_id, &body.endpoint)
        })
        .await
    {
        // Idempotent: removing an unknown endpoint is still a success.
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => ApiError::user_database(error).into_response(),
    }
}
