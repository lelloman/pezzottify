//! Named authorization policies used at HTTP route boundaries.

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
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

    if !session.has_permission(permission) {
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
            session: Session,
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
