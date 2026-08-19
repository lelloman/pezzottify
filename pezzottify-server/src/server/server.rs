//! HTTP server implementation with route handlers
//! Note: Many functions appear unused but are registered as route handlers

#![allow(dead_code)] // Route handlers registered dynamically

#[path = "route_builder.rs"]
mod route_builder;

use anyhow::{Context, Result};
use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use tracing::{debug, error, info, warn};

use crate::background_jobs::jobs::{
    CatalogAvailabilityStatsJob, CatalogAvailabilityStatsSnapshot, FeaturedAlbumsJob,
    FeaturedAlbumsSnapshot,
};
use crate::background_jobs::{JobError, JobInfo, SchedulerHandle};
use crate::catalog_store::{CatalogStore, DiscographySort};
use crate::config::{AlbumEmbeddingDerivationSpec, AudioEmbeddingSpec};
use crate::shows::{
    admin_routes as show_admin_routes, public_routes as show_public_routes, SqliteShowStore,
};
use crate::{
    server::stream_track::stream_track,
    user::{
        device::{DeviceRegistration, DeviceShareMode, DeviceSharePolicy},
        settings::UserSetting,
        sync_events::UserEvent,
        user_models::LikedContentType,
        FullUserStore, Permission, UserRole,
    },
};
use axum_extra::extract::cookie::CookieJar;
use tower_http::services::{ServeDir, ServeFile};

const AUDIO_EMBEDDING_COVERAGE_TIMEOUT: Duration = Duration::from_secs(20);

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, response, HeaderMap, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_governor::GovernorLayer;

use super::api_error::ApiError;
#[cfg(feature = "slowdown")]
use super::slowdown_request;
use super::{
    embeddings, extract_login_account_for_rate_limit, extract_user_id_for_rate_limit,
    http_api_no_store, http_cache, log_requests, make_search_admin_routes, make_search_routes,
    recommendation_routes, state::*, AnalyticsDeviceKeyExtractor, IpKeyExtractor,
    LoginAccountKeyExtractor, RequestsLoggingLevel, ServerConfig, UserOrIpKeyExtractor,
    ANALYTICS_PER_DEVICE_PER_MINUTE, CONTENT_READ_PER_MINUTE, GLOBAL_PER_MINUTE, LOGIN_PER_MINUTE,
    LOGIN_SUSTAINED_REPLENISH_MILLIS, SEARCH_PER_MINUTE, STREAM_PER_MINUTE, WRITE_PER_MINUTE,
};
use crate::server::session::Session;
use crate::server::session_cookie::{
    append_expired_session_cookies, append_session_cookies, require_csrf,
};
use crate::user::auth::AuthTokenValue;
use tower_governor::governor::GovernorConfigBuilder;

use super::authorization::{
    require_access_catalog, require_edit_catalog, require_like_content, require_manage_permissions,
    require_own_playlists, require_report_bug, require_request_content, require_server_admin,
    require_view_analytics,
};

pub(super) const MAX_DEVICES_PER_USER: usize = 6;

#[derive(Serialize)]
struct ServerStats {
    pub uptime: String,
    pub hash: String,
    pub session_token: Option<String>,
}

#[derive(Serialize)]
struct ArtistEnrichmentPayload {
    profile: crate::enrichment_store::ArtistEnrichmentV1,
    tags: Vec<crate::enrichment_store::EntityTagV1>,
    contributors: Vec<crate::enrichment_store::EntityContributorV1>,
    relations: Vec<crate::enrichment_store::EntityRelationV1>,
}

#[derive(Serialize)]
struct AlbumEnrichmentPayload {
    profile: crate::enrichment_store::AlbumEnrichmentV1,
    tags: Vec<crate::enrichment_store::EntityTagV1>,
    contributors: Vec<crate::enrichment_store::EntityContributorV1>,
    relations: Vec<crate::enrichment_store::EntityRelationV1>,
}

#[derive(Serialize)]
struct TrackEnrichmentPayload {
    profile: crate::enrichment_store::TrackEnrichmentV1,
    tags: Vec<crate::enrichment_store::EntityTagV1>,
    contributors: Vec<crate::enrichment_store::EntityContributorV1>,
    relations: Vec<crate::enrichment_store::EntityRelationV1>,
}

fn format_uptime(duration: Duration) -> String {
    let total_seconds = duration.as_secs();

    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
}

include!("handlers_sync.rs");
include!("handlers_catalog.rs");
include!("handlers_library.rs");
include!("handlers_account.rs");
include!("handlers_auth.rs");
include!("handlers_admin_operations.rs");
include!("handlers_admin_users.rs");
include!("bootstrap.rs");
