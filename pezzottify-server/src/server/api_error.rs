use axum::{
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

use crate::{catalog_store::CatalogMutationError, db_executor::DbRunError, user::UserServiceError};

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const RETRY_AFTER_SECONDS: &str = "1";

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    request_id: String,
    retry_after: Option<HeaderValue>,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    code: &'static str,
    message: String,
    request_id: String,
}

impl ApiError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn internal(context: &'static str, source: impl std::fmt::Display) -> Self {
        let api_error = Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "An internal error occurred",
        );
        error!(
            request_id = %api_error.request_id,
            error = %source,
            "{context}"
        );
        api_error
    }

    pub fn catalog_mutation(error: anyhow::Error) -> Self {
        match error.downcast::<CatalogMutationError>() {
            Ok(CatalogMutationError::AlreadyExists { entity, id }) => Self::conflict(
                "catalog_item_exists",
                format!("{entity} '{id}' already exists"),
            ),
            Ok(CatalogMutationError::NotFound { entity, id }) => Self::not_found(
                "catalog_item_not_found",
                format!("{entity} '{id}' not found"),
            ),
            Ok(CatalogMutationError::InvalidReference { entity, id }) => Self::bad_request(
                "invalid_catalog_reference",
                format!("Referenced {entity} '{id}' not found"),
            ),
            Err(source) => Self::internal("Catalog mutation failed", source),
        }
    }

    pub fn user_database(error: DbRunError) -> Self {
        match error {
            DbRunError::Store(source) => match source.downcast::<UserServiceError>() {
                Ok(error) => Self::from(error),
                Err(source) => Self::internal("User database operation failed", source),
            },
            executor => Self::from(executor),
        }
    }

    pub fn password_verification_unavailable() -> Self {
        let mut error = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_busy",
            "Authentication capacity is temporarily unavailable",
        );
        error.retry_after = Some(HeaderValue::from_static(RETRY_AFTER_SECONDS));
        error
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
            retry_after: None,
        }
    }

    fn database_unavailable() -> Self {
        let mut error = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
            "Database capacity is temporarily unavailable",
        );
        error.retry_after = Some(HeaderValue::from_static(RETRY_AFTER_SECONDS));
        error
    }
}

impl From<UserServiceError> for ApiError {
    fn from(error: UserServiceError) -> Self {
        match error {
            UserServiceError::Validation(message) => Self::bad_request("invalid_request", message),
            UserServiceError::NotFound(message) => Self::not_found("playlist_not_found", message),
            UserServiceError::Conflict(message) => Self::conflict("idempotency_conflict", message),
            UserServiceError::Internal(source) => {
                Self::internal("User service operation failed", source)
            }
        }
    }
}

impl From<DbRunError> for ApiError {
    fn from(error: DbRunError) -> Self {
        match error {
            DbRunError::QueueTimeout | DbRunError::ExecutionTimeout | DbRunError::ShuttingDown => {
                Self::database_unavailable()
            }
            internal => Self::internal("Database executor operation failed", internal),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = self.request_id.clone();
        let mut response = (
            self.status,
            Json(ApiErrorBody {
                code: self.code,
                message: self.message,
                request_id: self.request_id,
            }),
        )
            .into_response();
        response.headers_mut().insert(
            REQUEST_ID_HEADER,
            HeaderValue::from_str(&request_id).expect("UUID is a valid header value"),
        );
        if let Some(retry_after) = self.retry_after {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, retry_after);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_service_errors_have_stable_status_and_code() {
        let response = ApiError::from(UserServiceError::playlist_not_found()).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));

        let response = ApiError::from(UserServiceError::operation_conflict()).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[test]
    fn user_database_errors_preserve_domain_and_capacity_contracts() {
        let response = ApiError::user_database(DbRunError::Store(
            UserServiceError::playlist_not_found().into(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = ApiError::user_database(DbRunError::QueueTimeout).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()[axum::http::header::RETRY_AFTER],
            RETRY_AFTER_SECONDS
        );
    }

    #[tokio::test]
    async fn internal_errors_do_not_expose_their_source() {
        let response = ApiError::internal(
            "Test database operation failed",
            "SQLITE_CONSTRAINT users.secret_column",
        )
        .into_response();
        let request_id = response.headers()[REQUEST_ID_HEADER]
            .to_str()
            .unwrap()
            .to_owned();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body["code"], "internal_error");
        assert_eq!(body["message"], "An internal error occurred");
        assert_eq!(body["request_id"], request_id);
        assert!(!body.to_string().contains("secret_column"));
    }

    #[tokio::test]
    async fn executor_capacity_failures_have_a_stable_retryable_contract() {
        for error in [
            DbRunError::QueueTimeout,
            DbRunError::ExecutionTimeout,
            DbRunError::ShuttingDown,
        ] {
            let response = ApiError::from(error).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                response.headers()[axum::http::header::RETRY_AFTER],
                RETRY_AFTER_SECONDS
            );
            assert!(response.headers().contains_key(REQUEST_ID_HEADER));

            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["code"], "database_unavailable");
            assert_eq!(
                body["message"],
                "Database capacity is temporarily unavailable"
            );
        }
    }

    #[tokio::test]
    async fn password_verification_capacity_is_retryable_without_exposing_details() {
        let response = ApiError::password_verification_unavailable().into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()[axum::http::header::RETRY_AFTER],
            RETRY_AFTER_SECONDS
        );
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "authentication_busy");
        assert_eq!(
            body["message"],
            "Authentication capacity is temporarily unavailable"
        );
    }

    #[tokio::test]
    async fn executor_internal_failures_remain_opaque() {
        let response = ApiError::from(DbRunError::Store(anyhow::anyhow!(
            "SQLITE_CONSTRAINT users.secret_column"
        )))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response
            .headers()
            .contains_key(axum::http::header::RETRY_AFTER));

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "internal_error");
        assert!(!body.to_string().contains("secret_column"));
    }
}
