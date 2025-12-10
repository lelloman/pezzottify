//! Catalog ingestion for downloaded content.
//!
//! Converts external downloader types to catalog types and inserts them
//! into the CatalogStore.

use anyhow::Result;

use crate::catalog_store::{
    Album, AlbumType, Artist, ArtistRole, Format, Image, ImageSize, ImageType, Track,
    WritableCatalogStore,
};

use super::downloader_types::{ExternalAlbum, ExternalArtist, ExternalImage, ExternalTrack};

/// Result of ingesting an album - contains IDs needed for creating child queue items.
#[derive(Debug, Clone)]
pub struct IngestedAlbum {
    /// Album ID (base62)
    pub album_id: String,
    /// Track IDs (base62)
    pub track_ids: Vec<String>,
    /// Album cover image IDs (40-char hex)
    pub album_image_ids: Vec<String>,
    /// Artist portrait image IDs (40-char hex)
    pub artist_image_ids: Vec<String>,
}

/// Ingest an album and its related entities into the catalog.
///
/// # Logic:
/// 1. For each artist: check if exists, insert if not
/// 2. Insert album (links to artists)
/// 3. Insert tracks (links to album and artists)
/// 4. Insert images and link to artists/albums
/// 5. Return IDs needed for child queue item creation
pub fn ingest_album(
    catalog_store: &dyn WritableCatalogStore,
    album: &ExternalAlbum,
    tracks: &[ExternalTrack],
    artists: &[ExternalArtist],
) -> Result<IngestedAlbum> {
    // Extract IDs for return value
    let album_id = album.id.clone();
    let album_image_ids: Vec<String> = album.covers.iter().map(|c| c.id.clone()).collect();
    let track_ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    let artist_image_ids: Vec<String> = artists
        .iter()
        .flat_map(|a| a.portraits.iter().map(|p| p.id.clone()))
        .collect();

    // 1. For each artist: check if exists, insert if not
    for artist in artists.iter() {
        if !catalog_store.artist_exists(&artist.id)? {
            let catalog_artist = convert_artist(artist);
            catalog_store.insert_artist(&catalog_artist)?;
        }

        // Insert artist images
        for (img_pos, portrait) in artist.portraits.iter().enumerate() {
            if !catalog_store.image_exists(&portrait.id)? {
                let catalog_image = convert_image(portrait);
                catalog_store.insert_image(&catalog_image)?;
            }
            // Link image to artist (ignore error if already linked)
            let _ = catalog_store.add_artist_image(
                &artist.id,
                &portrait.id,
                &ImageType::Portrait,
                img_pos as i32,
            );
        }
    }

    // 2. Insert album FIRST (before linking artists - foreign key constraint)
    // Skip if album already exists (e.g., re-download or retry)
    if !catalog_store.album_exists(&album_id)? {
        let catalog_album = convert_album(&album);
        catalog_store.insert_album(&catalog_album)?;
    }

    // 3. Now link artists to album (album must exist first)
    for (position, artist) in artists.iter().enumerate() {
        // Link artist to album (ignore error if already linked)
        let _ = catalog_store.add_album_artist(&album_id, &artist.id, position as i32);
    }

    // 4. Insert album cover images
    for (position, cover) in album.covers.iter().enumerate() {
        if !catalog_store.image_exists(&cover.id)? {
            let catalog_image = convert_image(cover);
            catalog_store.insert_image(&catalog_image)?;
        }
        // Link image to album (ignore error if already linked)
        let _ = catalog_store.add_album_image(&album_id, &cover.id, &ImageType::Cover, position as i32);
    }

    // 5. Insert tracks (links to album and artists)
    // Skip tracks that already exist (e.g., re-download or retry)
    for track in tracks {
        if !catalog_store.track_exists(&track.id)? {
            let catalog_track = convert_track(track);
            catalog_store.insert_track(&catalog_track)?;
        }

        // Link artists to track - merge artists_with_role and artists_ids
        let mut seen_artist_ids = std::collections::HashSet::new();
        let mut has_main_artist = false;
        let mut position = 0i32;

        // First, add all artists with their actual roles
        for artist_with_role in &track.artists_with_role {
            let role = convert_artist_role(&artist_with_role.role);
            if role == ArtistRole::MainArtist {
                has_main_artist = true;
            }
            let _ = catalog_store.add_track_artist(
                &track.id,
                &artist_with_role.artist_id,
                &role,
                position,
            );
            seen_artist_ids.insert(artist_with_role.artist_id.clone());
            position += 1;
        }

        // Then, add any artists from artists_ids that weren't in artists_with_role
        for artist_id in &track.artists_ids {
            if !seen_artist_ids.contains(artist_id) {
                // If no main artist yet, first unseen artist becomes main
                let role = if !has_main_artist {
                    has_main_artist = true;
                    ArtistRole::MainArtist
                } else {
                    ArtistRole::FeaturedArtist
                };
                let _ = catalog_store.add_track_artist(&track.id, artist_id, &role, position);
                seen_artist_ids.insert(artist_id.clone());
                position += 1;
            }
        }
    }

    // 6. Return info needed for child creation
    Ok(IngestedAlbum {
        album_id,
        track_ids,
        album_image_ids,
        artist_image_ids,
    })
}

/// Convert external artist to catalog artist.
fn convert_artist(external: &ExternalArtist) -> Artist {
    Artist {
        id: external.id.clone(),
        name: external.name.clone(),
        genres: external.genre.clone(),
        activity_periods: vec![], // TODO: Convert activity_periods from external API
    }
}

/// Convert external album to catalog album.
fn convert_album(external: &ExternalAlbum) -> Album {
    Album {
        id: external.id.clone(),
        name: external.name.clone(),
        album_type: convert_album_type(&external.album_type),
        label: if external.label.is_empty() {
            None
        } else {
            Some(external.label.clone())
        },
        release_date: if external.date == 0 {
            None
        } else {
            Some(external.date)
        },
        genres: external.genres.clone(),
        original_title: external.original_title.clone(),
        version_title: if external.version_title.is_empty() {
            None
        } else {
            Some(external.version_title.clone())
        },
    }
}

/// Convert external track to catalog track.
fn convert_track(external: &ExternalTrack) -> Track {
    Track {
        id: external.id.clone(),
        name: external.name.clone(),
        album_id: external.album_id.clone(),
        disc_number: external.disc_number,
        track_number: external.number,
        duration_secs: Some((external.duration / 1000) as i32), // Convert ms to seconds
        is_explicit: external.is_explicit,
        audio_uri: format!("audio/{}.flac", external.id), // Default to flac, actual extension determined at download
        format: Format::Flac, // Default format, actual format determined at download
        tags: external.tags.clone(),
        has_lyrics: external.has_lyrics,
        languages: external.language_of_performance.clone(),
        original_title: external.original_title.clone(),
        version_title: if external.version_title.is_empty() {
            None
        } else {
            Some(external.version_title.clone())
        },
    }
}

/// Convert external image to catalog image.
fn convert_image(external: &ExternalImage) -> Image {
    Image {
        id: external.id.clone(),
        uri: format!("images/{}.jpg", external.id),
        size: convert_image_size(&external.size),
        width: external.width as u16,
        height: external.height as u16,
    }
}

/// Convert album type string to AlbumType enum.
fn convert_album_type(s: &str) -> AlbumType {
    match s.to_lowercase().as_str() {
        "album" => AlbumType::Album,
        "single" => AlbumType::Single,
        "ep" => AlbumType::Ep,
        "compilation" => AlbumType::Compilation,
        _ => AlbumType::Album,
    }
}

/// Convert image size string to ImageSize enum.
fn convert_image_size(s: &str) -> ImageSize {
    match s.to_lowercase().as_str() {
        "small" => ImageSize::Small,
        "medium" | "default" => ImageSize::Default,
        "large" => ImageSize::Large,
        "xlarge" | "xl" => ImageSize::XLarge,
        _ => ImageSize::Default,
    }
}

/// Convert external artist role string to ArtistRole enum.
fn convert_artist_role(s: &str) -> ArtistRole {
    match s.to_uppercase().as_str() {
        "MAIN_ARTIST" | "MAIN" | "PRIMARY" => ArtistRole::MainArtist,
        "FEATURED_ARTIST" | "FEATURED" | "FEAT" => ArtistRole::FeaturedArtist,
        "REMIXER" | "REMIX" => ArtistRole::Remixer,
        "COMPOSER" | "WRITER" | "SONGWRITER" => ArtistRole::Composer,
        "CONDUCTOR" => ArtistRole::Conductor,
        "ORCHESTRA" => ArtistRole::Orchestra,
        "ACTOR" => ArtistRole::Actor,
        _ => ArtistRole::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_album_type() {
        assert!(matches!(convert_album_type("album"), AlbumType::Album));
        assert!(matches!(convert_album_type("Album"), AlbumType::Album));
        assert!(matches!(convert_album_type("ALBUM"), AlbumType::Album));
        assert!(matches!(convert_album_type("single"), AlbumType::Single));
        assert!(matches!(convert_album_type("ep"), AlbumType::Ep));
        assert!(matches!(convert_album_type("compilation"), AlbumType::Compilation));
        assert!(matches!(convert_album_type("unknown"), AlbumType::Album));
    }

    #[test]
    fn test_convert_image_size() {
        assert!(matches!(convert_image_size("small"), ImageSize::Small));
        assert!(matches!(convert_image_size("medium"), ImageSize::Default));
        assert!(matches!(convert_image_size("large"), ImageSize::Large));
        assert!(matches!(convert_image_size("xlarge"), ImageSize::XLarge));
        assert!(matches!(convert_image_size("unknown"), ImageSize::Default));
    }

    #[test]
    fn test_convert_artist() {
        let external = ExternalArtist {
            id: "artist123".to_string(),
            name: "Test Artist".to_string(),
            genre: vec!["rock".to_string(), "pop".to_string()],
            portraits: vec![],
            activity_periods: vec![],
            related: vec![],
            portrait_group: vec![],
        };

        let catalog = convert_artist(&external);

        assert_eq!(catalog.id, "artist123");
        assert_eq!(catalog.name, "Test Artist");
        assert_eq!(catalog.genres, vec!["rock", "pop"]);
        assert!(catalog.activity_periods.is_empty());
    }

    #[test]
    fn test_convert_album() {
        let external = ExternalAlbum {
            id: "album123".to_string(),
            name: "Test Album".to_string(),
            album_type: "album".to_string(),
            artists_ids: vec!["artist1".to_string()],
            label: "Test Label".to_string(),
            date: 1704067200,
            genres: vec!["rock".to_string()],
            covers: vec![],
            discs: vec![],
            related: vec![],
            cover_group: vec![],
            original_title: Some("Original Title".to_string()),
            version_title: "Deluxe".to_string(),
            type_str: String::new(),
        };

        let catalog = convert_album(&external);

        assert_eq!(catalog.id, "album123");
        assert_eq!(catalog.name, "Test Album");
        assert!(matches!(catalog.album_type, AlbumType::Album));
        assert_eq!(catalog.label, Some("Test Label".to_string()));
        assert_eq!(catalog.release_date, Some(1704067200));
        assert_eq!(catalog.genres, vec!["rock"]);
        assert_eq!(catalog.original_title, Some("Original Title".to_string()));
        assert_eq!(catalog.version_title, Some("Deluxe".to_string()));
    }

    #[test]
    fn test_convert_track() {
        use std::collections::HashMap;

        let external = ExternalTrack {
            id: "track123".to_string(),
            name: "Test Track".to_string(),
            album_id: "album123".to_string(),
            artists_ids: vec!["artist1".to_string()],
            number: 1,
            disc_number: 1,
            duration: 180000, // 180 seconds in ms
            is_explicit: true,
            files: HashMap::new(),
            alternatives: vec![],
            tags: vec!["live".to_string()],
            earliest_live_timestamp: None,
            has_lyrics: true,
            language_of_performance: vec!["en".to_string()],
            original_title: None,
            version_title: String::new(),
            artists_with_role: vec![],
        };

        let catalog = convert_track(&external);

        assert_eq!(catalog.id, "track123");
        assert_eq!(catalog.name, "Test Track");
        assert_eq!(catalog.album_id, "album123");
        assert_eq!(catalog.disc_number, 1);
        assert_eq!(catalog.track_number, 1);
        assert_eq!(catalog.duration_secs, Some(180));
        assert!(catalog.is_explicit);
        assert_eq!(catalog.audio_uri, "audio/track123.flac");
        assert_eq!(catalog.tags, vec!["live"]);
        assert!(catalog.has_lyrics);
        assert_eq!(catalog.languages, vec!["en"]);
    }

    #[test]
    fn test_convert_image() {
        let external = ExternalImage {
            id: "abc123def456".to_string(),
            size: "large".to_string(),
            width: 640,
            height: 640,
        };

        let catalog = convert_image(&external);

        assert_eq!(catalog.id, "abc123def456");
        assert_eq!(catalog.uri, "images/abc123def456.jpg");
        assert!(matches!(catalog.size, ImageSize::Large));
        assert_eq!(catalog.width, 640);
        assert_eq!(catalog.height, 640);
    }
}
