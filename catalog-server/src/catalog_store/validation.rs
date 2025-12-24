//! Validation for catalog entities.
//!
//! Provides validation functions to ensure data integrity before
//! inserting or updating entities in the catalog store.
#![allow(dead_code)]

use super::models::{Album, Artist, Image, Track, TrackAvailability};
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
        value: i32,
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
    if track.audio_uri.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "audio_uri" });
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
    if let Some(duration) = track.duration_secs {
        if duration < 0 {
            return Err(ValidationError::NegativeValue {
                field: "duration_secs",
                value: duration,
            });
        }
    }
    Ok(())
}

/// Validate an image entity
pub fn validate_image(image: &Image) -> ValidationResult<()> {
    if image.id.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "id" });
    }
    if image.uri.trim().is_empty() {
        return Err(ValidationError::EmptyField { field: "uri" });
    }
    if image.width == 0 {
        return Err(ValidationError::NonPositiveValue {
            field: "width",
            value: 0,
        });
    }
    if image.height == 0 {
        return Err(ValidationError::NonPositiveValue {
            field: "height",
            value: 0,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::models::{ActivityPeriod, AlbumType, Format, ImageSize};

    fn make_valid_artist() -> Artist {
        Artist {
            id: "artist-1".to_string(),
            name: "Test Artist".to_string(),
            genres: vec!["rock".to_string()],
            activity_periods: vec![ActivityPeriod::Decade(1990)],
        }
    }

    fn make_valid_album() -> Album {
        Album {
            id: "album-1".to_string(),
            name: "Test Album".to_string(),
            album_type: AlbumType::Album,
            label: None,
            release_date: None,
            genres: vec![],
            original_title: None,
            version_title: None,
        }
    }

    fn make_valid_track() -> Track {
        Track {
            id: "track-1".to_string(),
            name: "Test Track".to_string(),
            album_id: "album-1".to_string(),
            disc_number: 1,
            track_number: 1,
            duration_secs: Some(180),
            is_explicit: false,
            audio_uri: "albums/album-1/track-1.mp3".to_string(),
            format: Format::Mp3_320,
            tags: vec![],
            has_lyrics: false,
            languages: vec![],
            original_title: None,
            version_title: None,
            availability: TrackAvailability::Available,
        }
    }

    fn make_valid_image() -> Image {
        Image {
            id: "image-1".to_string(),
            uri: "images/image-1.jpg".to_string(),
            size: ImageSize::Default,
            width: 300,
            height: 300,
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
        track.duration_secs = Some(-10);
        let err = validate_track(&track).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NegativeValue {
                field: "duration_secs",
                ..
            }
        ));
    }

    #[test]
    fn test_validate_image_valid() {
        let image = make_valid_image();
        assert!(validate_image(&image).is_ok());
    }

    #[test]
    fn test_validate_image_zero_width() {
        let mut image = make_valid_image();
        image.width = 0;
        let err = validate_image(&image).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NonPositiveValue { field: "width", .. }
        ));
    }

    #[test]
    fn test_validate_image_zero_height() {
        let mut image = make_valid_image();
        image.height = 0;
        let err = validate_image(&image).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::NonPositiveValue {
                field: "height",
                ..
            }
        ));
    }
}
