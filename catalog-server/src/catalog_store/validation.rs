//! Validation for catalog entities.
//!
//! Provides validation functions to ensure data integrity before
//! inserting or updating entities in the catalog store.

use super::models::{Album, Artist, Track};
use std::fmt;

/// Validation error types
#[derive(Debug)]
pub enum ValidationError {
    EmptyField {
        field: &'static str,
    },
    NonPositiveValue {
        field: &'static str,
        value: i32,
    },
    NegativeValue {
        field: &'static str,
        value: i64,
    },
    ForeignKeyViolation {
        entity_type: &'static str,
        id: String,
    },
    DuplicateId {
        entity_type: &'static str,
        id: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::EmptyField { field } => {
                write!(f, "Field '{}' is required but was empty", field)
            }
            ValidationError::NonPositiveValue { field, value } => {
                write!(f, "Field '{}' must be positive, got {}", field, value)
            }
            ValidationError::NegativeValue { field, value } => {
                write!(f, "Field '{}' must be non-negative, got {}", field, value)
            }
            ValidationError::ForeignKeyViolation { entity_type, id } => {
                write!(f, "Referenced {} '{}' does not exist", entity_type, id)
            }
            ValidationError::DuplicateId { entity_type, id } => {
                write!(f, "{} with id '{}' already exists", entity_type, id)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validate an artist entity
pub fn validate_artist(artist: &Artist) -> ValidationResult<()> {
    if artist.id.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "id" });
    }
    if artist.name.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "name" });
    }
    Ok(())
}

/// Validate an album entity
pub fn validate_album(album: &Album) -> ValidationResult<()> {
    if album.id.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "id" });
    }
    if album.name.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "name" });
    }
    Ok(())
}

/// Validate a track entity (without foreign key check)
pub fn validate_track(track: &Track) -> ValidationResult<()> {
    if track.id.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "id" });
    }
    if track.name.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "name" });
    }
    if track.album_id.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "album_id" });
    }
    if track.disc_number < 1 {
        return Err(ValidationError::NonPositiveValue {
            field: "disc_number",
            value: track.disc_number,
        });
    }
    if track.track_number < 1 {
        return Err(ValidationError::NonPositiveValue {
            field: "track_number",
            value: track.track_number,
        });
    }
    if track.duration_ms < 0 {
        return Err(ValidationError::NegativeValue {
            field: "duration_ms",
            value: track.duration_ms,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::models::AlbumType;

    fn make_valid_artist() -> Artist {
        Artist {
            id: "artist-1".to_string(),
            name: "Test Artist".to_string(),
            genres: vec!["rock".to_string()],
            followers_total: 1000,
            popularity: 50,
        }
    }

    fn make_valid_album() -> Album {
        Album {
            id: "album-1".to_string(),
            name: "Test Album".to_string(),
            album_type: AlbumType::Album,
            label: None,
            release_date: Some("2023-01-01".to_string()),
            release_date_precision: Some("day".to_string()),
            external_id_upc: None,
            popularity: 50,
        }
    }

    fn make_valid_track() -> Track {
        Track {
            id: "track-1".to_string(),
            name: "Test Track".to_string(),
            album_id: "album-1".to_string(),
            disc_number: 1,
            track_number: 1,
            duration_ms: 180000,
            explicit: false,
            popularity: 50,
            language: None,
            external_id_isrc: None,
        }
    }

    #[test]
    fn test_validate_artist_valid() {
        let artist = make_valid_artist();
        assert!(validate_artist(&artist).is_ok());
    }

    #[test]
    fn test_validate_artist_empty_id() {
        let mut artist = make_valid_artist();
        artist.id = "".to_string();
        let err = validate_artist(&artist).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyField { field: "id" }));
    }

    #[test]
    fn test_validate_artist_empty_name() {
        let mut artist = make_valid_artist();
        artist.name = "  ".to_string(); // whitespace only
        let err = validate_artist(&artist).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyField { field: "name" }));
    }

    #[test]
    fn test_validate_album_valid() {
        let album = make_valid_album();
        assert!(validate_album(&album).is_ok());
    }

    #[test]
    fn test_validate_album_empty_id() {
        let mut album = make_valid_album();
        album.id = "".to_string();
        let err = validate_album(&album).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyField { field: "id" }));
    }

    #[test]
    fn test_validate_track_valid() {
        let track = make_valid_track();
        assert!(validate_track(&track).is_ok());
    }

    #[test]
    fn test_validate_track_empty_album_id() {
        let mut track = make_valid_track();
        track.album_id = "".to_string();
        let err = validate_track(&track).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::EmptyField { field: "album_id" }
        ));
    }

    #[test]
    fn test_validate_track_zero_disc_number() {
        let mut track = make_valid_track();
        track.disc_number = 0;
        let err = validate_track(&track).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NonPositiveValue {
                field: "disc_number",
                ..
            }
        ));
    }

    #[test]
    fn test_validate_track_negative_track_number() {
        let mut track = make_valid_track();
        track.track_number = -1;
        let err = validate_track(&track).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NonPositiveValue {
                field: "track_number",
                ..
            }
        ));
    }

    #[test]
    fn test_validate_track_negative_duration() {
        let mut track = make_valid_track();
        track.duration_ms = -10;
        let err = validate_track(&track).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NegativeValue {
                field: "duration_ms",
                ..
            }
        ));
    }
}
