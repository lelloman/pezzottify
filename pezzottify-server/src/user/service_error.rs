use thiserror::Error;

pub type ServiceResult<T> = std::result::Result<T, UserServiceError>;

/// Stable failures produced by user-facing service operations.
#[derive(Debug, Error)]
pub enum UserServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Internal service failure")]
    Internal(#[source] anyhow::Error),
}

impl UserServiceError {
    pub fn playlist_not_found() -> Self {
        Self::NotFound("Playlist not found".to_owned())
    }

    pub fn operation_conflict() -> Self {
        Self::Conflict("Idempotency key was already used for another operation".to_owned())
    }
}
