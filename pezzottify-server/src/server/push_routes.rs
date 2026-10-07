//! UnifiedPush registration routes. See docs/unifiedpush.md.

use crate::web::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use simple_server::extract::Extract;

use super::api_error::ApiError;
use super::session::Session;
use super::state::ServerState;
use crate::db_executor::DbPriority;
use crate::push::{endpoint_host, validate_endpoint, validate_keys, DeliveryOutcome, PushService};
use crate::user::PushRegistrationOverview;
use sha2::{Digest, Sha256};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const TEST_TITLE_MAX_CHARS: usize = 100;
const TEST_BODY_MAX_CHARS: usize = 300;
const DEFAULT_TEST_TITLE: &str = "Test notification";
const DEFAULT_TEST_BODY: &str = "Sent from the Pezzottify admin panel";

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

/// Admin routes, mounted under `/v1/admin` behind `require_server_admin`.
pub(crate) fn admin_push_routes() -> Router<ServerState> {
    Router::new()
        .route("/push/registrations", get(admin_list_registrations))
        .route("/push/registrations/{id}/test", post(admin_send_test))
}

#[derive(Debug, Serialize)]
struct AdminRegistration {
    id: String,
    user_id: usize,
    user_handle: Option<String>,
    device_uuid: Option<String>,
    device_name: Option<String>,
    device_type: Option<String>,
    endpoint_host: String,
    created_at: i64,
    last_success_at: Option<i64>,
    first_failure_at: Option<i64>,
    connected: bool,
}

#[derive(Debug, Serialize)]
struct AdminRegistrationsResponse {
    enabled: bool,
    registrations: Vec<AdminRegistration>,
}

#[derive(Debug, Default, Deserialize)]
struct TestNotificationBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct TestNotificationResponse {
    outcome: &'static str,
    detail: Option<String>,
}

/// Stable public id of a registration. The endpoint is a push capability and is
/// never exposed, so registrations are addressed by a prefix of its hash.
fn registration_id(endpoint: &str) -> String {
    Sha256::digest(endpoint.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Trimmed `value`, `default` when missing or blank, or an error past `max_chars`.
fn test_text(
    value: Option<String>,
    default: &str,
    max_chars: usize,
    what: &str,
) -> Result<String, ApiError> {
    let value = value.map(|v| v.trim().to_string()).unwrap_or_default();
    if value.is_empty() {
        return Ok(default.to_string());
    }
    if value.chars().count() > max_chars {
        return Err(ApiError::bad_request(
            "invalid_test_notification",
            format!("{what} must be at most {max_chars} characters"),
        ));
    }
    Ok(value)
}

async fn all_registrations(state: &ServerState) -> Result<Vec<PushRegistrationOverview>, ApiError> {
    state
        .database
        .user_store
        .run(DbPriority::Interactive, |store| {
            store.list_all_push_registrations()
        })
        .await
        .map_err(ApiError::user_database)
}

async fn admin_list_registrations(State(state): State<ServerState>) -> Response {
    if state.push.is_none() {
        return Json(AdminRegistrationsResponse {
            enabled: false,
            registrations: Vec::new(),
        })
        .into_response();
    }
    let overviews = match all_registrations(&state).await {
        Ok(overviews) => overviews,
        Err(error) => return error.into_response(),
    };
    let mut connected = HashMap::<usize, HashSet<usize>>::new();
    for overview in &overviews {
        let user_id = overview.registration.user_id;
        if let Entry::Vacant(entry) = connected.entry(user_id) {
            let devices = state
                .ws_connection_manager
                .get_connected_devices(user_id)
                .await;
            entry.insert(devices.into_iter().collect());
        }
    }
    let registrations = overviews
        .into_iter()
        .map(|overview| {
            let registration = overview.registration;
            let is_connected = overview.device_row_id.is_some_and(|device| {
                connected
                    .get(&registration.user_id)
                    .is_some_and(|devices| devices.contains(&device))
            });
            AdminRegistration {
                id: registration_id(&registration.endpoint),
                user_id: registration.user_id,
                user_handle: overview.user_handle,
                device_uuid: registration.device_id,
                device_name: overview.device_name,
                device_type: overview.device_type,
                endpoint_host: endpoint_host(&registration.endpoint),
                created_at: registration.created_at,
                last_success_at: registration.last_success_at,
                first_failure_at: registration.first_failure_at,
                connected: is_connected,
            }
        })
        .collect();
    Json(AdminRegistrationsResponse {
        enabled: true,
        registrations,
    })
    .into_response()
}

async fn admin_send_test(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<TestNotificationBody>,
) -> Response {
    let push = match service(&state) {
        Ok(push) => push.clone(),
        Err(error) => return error.into_response(),
    };
    let (title, text) = match test_text(
        body.title,
        DEFAULT_TEST_TITLE,
        TEST_TITLE_MAX_CHARS,
        "title",
    )
    .and_then(|title| {
        test_text(body.body, DEFAULT_TEST_BODY, TEST_BODY_MAX_CHARS, "body")
            .map(|text| (title, text))
    }) {
        Ok(texts) => texts,
        Err(error) => return error.into_response(),
    };
    let registration = match all_registrations(&state).await {
        Ok(overviews) => overviews
            .into_iter()
            .map(|overview| overview.registration)
            .find(|registration| registration_id(&registration.endpoint) == id),
        Err(error) => return error.into_response(),
    };
    let Some(registration) = registration else {
        return ApiError::not_found("push_registration_not_found", "No such push registration")
            .into_response();
    };
    let (outcome, detail) = match push.send_test(&registration, &title, &text).await {
        DeliveryOutcome::Delivered => ("delivered", None),
        DeliveryOutcome::Gone => (
            "gone",
            Some("The push service no longer knows this endpoint; registration removed".into()),
        ),
        DeliveryOutcome::Failed(reason) => ("failed", Some(reason)),
    };
    Json(TestNotificationResponse { outcome, detail }).into_response()
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
