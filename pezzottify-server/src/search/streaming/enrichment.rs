//! Enrichment data conversions for streaming search.
//!
//! Provides conversions from full catalog models to lightweight summary types
//! used in streaming search enrichment sections.

use crate::catalog_store::{
    Album, Artist, ResolvedAlbum, ResolvedArtist, ResolvedTrack, TrackAvailability,
};

use super::sections::{AlbumSummary, ArtistSummary, TrackSummary};

/// Convert a ResolvedTrack to TrackSummary.
impl From<&ResolvedTrack> for TrackSummary {
    fn from(resolved: &ResolvedTrack) -> Self {
        let artist_names: Vec<String> = resolved
            .artists
            .iter()
            .map(|ta| ta.artist.name.clone())
            .collect();

        TrackSummary {
            id: resolved.track.id.clone(),
            name: resolved.track.name.clone(),
            duration_ms: resolved.track.duration_ms as u64,
            track_number: Some(resolved.track.track_number as u32),
            album_id: resolved.album.id.clone(),
            album_name: resolved.album.name.clone(),
            artist_names,
            image_id: Some(resolved.album.id.clone()), // Use album ID as image ID
            available: resolved.track.availability == TrackAvailability::Available,
        }
    }
}

/// Convert a ResolvedTrack with album image to TrackSummary.
pub fn track_summary_with_image(resolved: &ResolvedTrack, image_id: Option<&str>) -> TrackSummary {
    let mut summary = TrackSummary::from(resolved);
    summary.image_id = image_id.map(|s| s.to_string());
    summary
}

/// Convert a ResolvedAlbum to AlbumSummary.
impl From<&ResolvedAlbum> for AlbumSummary {
    fn from(resolved: &ResolvedAlbum) -> Self {
        // Extract release year from string date (e.g., "2023-05-15", "2023-05", "2023")
        let release_year = resolved
            .album
            .release_date
            .as_ref()
            .and_then(|date| date.split('-').next().and_then(|y| y.parse::<i32>().ok()));

        // Count all tracks across all discs
        let track_count: u32 = resolved.discs.iter().map(|d| d.tracks.len() as u32).sum();

        let artist_names: Vec<String> = resolved.artists.iter().map(|a| a.name.clone()).collect();

        AlbumSummary {
            id: resolved.album.id.clone(),
            name: resolved.album.name.clone(),
            release_year,
            track_count,
            image_id: Some(resolved.album.id.clone()), // Use album ID as image ID
            artist_names,
            availability: resolved.album.album_availability.to_db_str().to_string(),
        }
    }
}

/// Convert an Album to AlbumSummary (without resolved data).
/// Note: track_count will be 0, availability will be 'missing'.
pub fn album_summary_basic(album: &Album, artist_names: Vec<String>) -> AlbumSummary {
    let release_year = album
        .release_date
        .as_ref()
        .and_then(|date| date.split('-').next().and_then(|y| y.parse::<i32>().ok()));

    AlbumSummary {
        id: album.id.clone(),
        name: album.name.clone(),
        release_year,
        track_count: 0,                   // Unknown without resolved data
        image_id: Some(album.id.clone()), // Use album ID as image ID
        artist_names,
        availability: album.album_availability.to_db_str().to_string(),
    }
}

/// Convert a ResolvedArtist to ArtistSummary.
impl From<&ResolvedArtist> for ArtistSummary {
    fn from(resolved: &ResolvedArtist) -> Self {
        ArtistSummary {
            id: resolved.artist.id.clone(),
            name: resolved.artist.name.clone(),
            image_id: Some(resolved.artist.id.clone()), // Use artist ID as image ID
            available: resolved.artist.available,
        }
    }
}

/// Convert an Artist to ArtistSummary.
impl From<&Artist> for ArtistSummary {
    fn from(artist: &Artist) -> Self {
        ArtistSummary {
            id: artist.id.clone(),
            name: artist.name.clone(),
            image_id: Some(artist.id.clone()), // Use artist ID as image ID
            available: artist.available,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::{
        AlbumAvailability, AlbumType, ArtistRole, Disc, Track, TrackArtist, TrackAvailability,
    };

    fn make_artist(id: &str, name: &str) -> Artist {
        Artist {
            id: id.to_string(),
            name: name.to_string(),
            genres: vec![],
            followers_total: 0,
            popularity: 0,
            available: false,
        }
    }

    fn make_album(id: &str, name: &str) -> Album {
        Album {
            id: id.to_string(),
            name: name.to_string(),
            album_type: AlbumType::Album,
            label: None,
            release_date: Some("1984-01-01".to_string()),
            release_date_precision: Some("day".to_string()),
            external_id_upc: None,
            popularity: 0,
            album_availability: AlbumAvailability::default(),
        }
    }

    fn make_track(id: &str, name: &str, album_id: &str) -> Track {
        Track {
            id: id.to_string(),
            name: name.to_string(),
            album_id: album_id.to_string(),
            disc_number: 1,
            track_number: 1,
            duration_ms: 240000,
            explicit: false,
            popularity: 0,
            language: None,
            external_id_isrc: None,
            audio_uri: None,
            availability: TrackAvailability::default(),
        }
    }

    #[test]
    fn test_resolved_track_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let album = make_album("album_1", "Purple Rain");
        let track = make_track("track_1", "When Doves Cry", "album_1");

        let resolved = ResolvedTrack {
            track,
            album,
            artists: vec![TrackArtist {
                artist,
                role: ArtistRole::MainArtist,
            }],
        };

        let summary = TrackSummary::from(&resolved);

        assert_eq!(summary.id, "track_1");
        assert_eq!(summary.name, "When Doves Cry");
        assert_eq!(summary.duration_ms, 240000);
        assert_eq!(summary.track_number, Some(1));
        assert_eq!(summary.album_id, "album_1");
        assert_eq!(summary.album_name, "Purple Rain");
        assert_eq!(summary.artist_names, vec!["Prince"]);
        assert_eq!(summary.image_id, Some("album_1".to_string()));
    }

    #[test]
    fn test_track_summary_with_image() {
        let artist = make_artist("artist_1", "Prince");
        let album = make_album("album_1", "Purple Rain");
        let track = make_track("track_1", "When Doves Cry", "album_1");

        let resolved = ResolvedTrack {
            track,
            album,
            artists: vec![TrackArtist {
                artist,
                role: ArtistRole::MainArtist,
            }],
        };

        let summary = track_summary_with_image(&resolved, Some("custom_img"));

        assert_eq!(summary.image_id, Some("custom_img".to_string()));
    }

    #[test]
    fn test_resolved_album_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let album = make_album("album_1", "Purple Rain");
        let track1 = make_track("track_1", "When Doves Cry", "album_1");
        let track2 = make_track("track_2", "Purple Rain", "album_1");

        let resolved = ResolvedAlbum {
            album,
            artists: vec![artist],
            discs: vec![Disc {
                number: 1,
                tracks: vec![track1, track2],
            }],
        };

        let summary = AlbumSummary::from(&resolved);

        assert_eq!(summary.id, "album_1");
        assert_eq!(summary.name, "Purple Rain");
        assert_eq!(summary.release_year, Some(1984));
        assert_eq!(summary.track_count, 2);
        assert_eq!(summary.image_id, Some("album_1".to_string()));
        assert_eq!(summary.artist_names, vec!["Prince"]);
        assert_eq!(summary.availability, "missing");
    }

    #[test]
    fn test_album_summary_basic() {
        let album = make_album("album_1", "Purple Rain");
        let summary = album_summary_basic(&album, vec!["Prince".to_string()]);

        assert_eq!(summary.id, "album_1");
        assert_eq!(summary.name, "Purple Rain");
        assert_eq!(summary.release_year, Some(1984));
        assert_eq!(summary.track_count, 0);
        assert_eq!(summary.image_id, Some("album_1".to_string()));
        assert_eq!(summary.artist_names, vec!["Prince"]);
        assert_eq!(summary.availability, "missing");
    }

    #[test]
    fn test_resolved_artist_to_summary() {
        let artist = make_artist("artist_1", "Prince");

        let resolved = ResolvedArtist {
            artist,
            related_artists: vec![],
        };

        let summary = ArtistSummary::from(&resolved);

        assert_eq!(summary.id, "artist_1");
        assert_eq!(summary.name, "Prince");
        assert_eq!(summary.image_id, Some("artist_1".to_string()));
    }

    #[test]
    fn test_artist_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let summary = ArtistSummary::from(&artist);

        assert_eq!(summary.id, "artist_1");
        assert_eq!(summary.name, "Prince");
        assert_eq!(summary.image_id, Some("artist_1".to_string()));
    }

    #[test]
    fn test_multiple_artists_on_track() {
        let artist1 = make_artist("artist_1", "Prince");
        let artist2 = make_artist("artist_2", "The Revolution");
        let album = make_album("album_1", "Purple Rain");
        let track = make_track("track_1", "When Doves Cry", "album_1");

        let resolved = ResolvedTrack {
            track,
            album,
            artists: vec![
                TrackArtist {
                    artist: artist1,
                    role: ArtistRole::MainArtist,
                },
                TrackArtist {
                    artist: artist2,
                    role: ArtistRole::FeaturedArtist,
                },
            ],
        };

        let summary = TrackSummary::from(&resolved);

        assert_eq!(summary.artist_names, vec!["Prince", "The Revolution"]);
    }
}
