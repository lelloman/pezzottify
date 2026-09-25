//! Named authorization policies used at HTTP route boundaries.

use simple_server::auth::Access;
use simple_server::axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use simple_server::extract::Extract;
use tracing::debug;

use crate::user::Permission;

use super::session::Session;

async fn require_permission(
    policy: &'static str,
    permission: Permission,
    session: Session,
    request: Request<Body>,
    next: Next,
) -> Response {
    debug!(
        policy,
        user_id = session.user_id,
        ?permission,
        permissions = ?session.permissions,
        "checking route permission"
    );

    // The extractor has already verified this session and loaded its permission
    // snapshot. Route policy operates on that same snapshot for this request.
    let access = Access::new(|session: &Session| Ok::<_, ()>(session.user_id))
        .with_check(move |_, session| session.has_permission(permission).then_some(()).ok_or(()));
    if access.evaluate(&session).is_err() {
        debug!(
            policy,
            user_id = session.user_id,
            ?permission,
            "route permission denied"
        );
        return StatusCode::FORBIDDEN.into_response();
    }

    next.run(request).await
}

macro_rules! permission_policy {
    ($name:ident, $permission:ident) => {
        pub(super) async fn $name(
            Extract(session): Extract<Session>,
            request: Request<Body>,
            next: Next,
        ) -> Response {
            require_permission(
                stringify!($name),
                Permission::$permission,
                session,
                request,
                next,
            )
            .await
        }
    };
}

permission_policy!(require_access_catalog, AccessCatalog);
permission_policy!(require_like_content, LikeContent);
permission_policy!(require_own_playlists, OwnPlaylists);
permission_policy!(require_edit_catalog, EditCatalog);
permission_policy!(require_server_admin, ServerAdmin);
permission_policy!(require_manage_permissions, ManagePermissions);
permission_policy!(require_view_analytics, ViewAnalytics);
permission_policy!(require_request_content, RequestContent);
permission_policy!(require_report_bug, ReportBug);
