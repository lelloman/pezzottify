use thiserror::Error;

/// Expected catalog mutation failures that callers can map without parsing text.
#[derive(Debug, Error)]
pub enum CatalogMutationError {
    #[error("{entity} '{id}' already exists")]
    AlreadyExists { entity: &'static str, id: String },
    #[error("{entity} '{id}' not found")]
    NotFound { entity: &'static str, id: String },
    #[error("Referenced {entity} '{id}' not found")]
    InvalidReference { entity: &'static str, id: String },
}
