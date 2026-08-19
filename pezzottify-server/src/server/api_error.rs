use axum::{
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

use crate::{catalog_store::CatalogMutationError, user::UserServiceError};

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    request_id: String,
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

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
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
}
