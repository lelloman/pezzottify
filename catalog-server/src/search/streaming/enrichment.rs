//! Enrichment data conversions for streaming search.
//!
//! Provides conversions from full catalog models to lightweight summary types
//! used in streaming search enrichment sections.

use crate::catalog_store::{Album, Artist, Image, ResolvedAlbum, ResolvedArtist, ResolvedTrack};

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
            duration_ms: resolved
                .track
                .duration_secs
                .map(|s| s as u64 * 1000)
                .unwrap_or(0),
            track_number: Some(resolved.track.track_number as u32),
            album_id: resolved.album.id.clone(),
            album_name: resolved.album.name.clone(),
            artist_names,
            image_id: None, // Would need to fetch album image separately
        }
    }
}

/// Convert a ResolvedTrack with album image to TrackSummary.
pub fn track_summary_with_image(resolved: &ResolvedTrack, image: Option<&Image>) -> TrackSummary {
    let mut summary = TrackSummary::from(resolved);
    summary.image_id = image.map(|i| i.id.clone());
    summary
}

/// Convert a ResolvedAlbum to AlbumSummary.
impl From<&ResolvedAlbum> for AlbumSummary {
    fn from(resolved: &ResolvedAlbum) -> Self {
        // Extract release year from timestamp
        let release_year = resolved.album.release_date.map(|ts| {
            // Convert Unix timestamp to year
            let datetime = chrono::DateTime::from_timestamp(ts, 0);
            datetime
                .map(|dt| dt.format("%Y").to_string().parse::<i32>().unwrap_or(0))
                .unwrap_or(0)
        });

        // Count all tracks across all discs
        let track_count: u32 = resolved.discs.iter().map(|d| d.tracks.len() as u32).sum();

        let artist_names: Vec<String> = resolved.artists.iter().map(|a| a.name.clone()).collect();

        AlbumSummary {
            id: resolved.album.id.clone(),
            name: resolved.album.name.clone(),
            release_year,
            track_count,
            image_id: resolved.display_image.as_ref().map(|i| i.id.clone()),
            artist_names,
        }
    }
}

/// Convert an Album to AlbumSummary (without resolved data).
/// Note: track_count will be 0 and image_id will be None.
pub fn album_summary_basic(album: &Album, artist_names: Vec<String>) -> AlbumSummary {
    let release_year = album.release_date.map(|ts| {
        let datetime = chrono::DateTime::from_timestamp(ts, 0);
        datetime
            .map(|dt| dt.format("%Y").to_string().parse::<i32>().unwrap_or(0))
            .unwrap_or(0)
    });

    AlbumSummary {
        id: album.id.clone(),
        name: album.name.clone(),
        release_year,
        track_count: 0, // Unknown without resolved data
        image_id: None, // Unknown without resolved data
        artist_names,
    }
}

/// Convert a ResolvedArtist to ArtistSummary.
impl From<&ResolvedArtist> for ArtistSummary {
    fn from(resolved: &ResolvedArtist) -> Self {
        ArtistSummary {
            id: resolved.artist.id.clone(),
            name: resolved.artist.name.clone(),
            image_id: resolved.display_image.as_ref().map(|i| i.id.clone()),
        }
    }
}

/// Convert an Artist to ArtistSummary (without display image).
impl From<&Artist> for ArtistSummary {
    fn from(artist: &Artist) -> Self {
        ArtistSummary {
            id: artist.id.clone(),
            name: artist.name.clone(),
            image_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::{
        AlbumType, ArtistRole, Disc, Format, Track, TrackArtist, TrackAvailability,
    };

    fn make_artist(id: &str, name: &str) -> Artist {
        Artist {
            id: id.to_string(),
            name: name.to_string(),
            genres: vec![],
            activity_periods: vec![],
        }
    }

    fn make_album(id: &str, name: &str) -> Album {
        Album {
            id: id.to_string(),
            name: name.to_string(),
            album_type: AlbumType::Album,
            label: None,
            release_date: Some(441763200), // 1984-01-01
            genres: vec![],
            original_title: None,
            version_title: None,
        }
    }

    fn make_track(id: &str, name: &str, album_id: &str) -> Track {
        Track {
            id: id.to_string(),
            name: name.to_string(),
            album_id: album_id.to_string(),
            disc_number: 1,
            track_number: 1,
            duration_secs: Some(240),
            is_explicit: false,
            audio_uri: "test.ogg".to_string(),
            format: Format::OggVorbis320,
            tags: vec![],
            has_lyrics: false,
            languages: vec![],
            original_title: None,
            version_title: None,
            availability: TrackAvailability::Available,
        }
    }

    fn make_image(id: &str) -> Image {
        Image {
            id: id.to_string(),
            uri: format!("{}.jpg", id),
            size: crate::catalog_store::ImageSize::Large,
            width: 640,
            height: 640,
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
        assert!(summary.image_id.is_none());
    }

    #[test]
    fn test_track_summary_with_image() {
        let artist = make_artist("artist_1", "Prince");
        let album = make_album("album_1", "Purple Rain");
        let track = make_track("track_1", "When Doves Cry", "album_1");
        let image = make_image("img_1");

        let resolved = ResolvedTrack {
            track,
            album,
            artists: vec![TrackArtist {
                artist,
                role: ArtistRole::MainArtist,
            }],
        };

        let summary = track_summary_with_image(&resolved, Some(&image));

        assert_eq!(summary.image_id, Some("img_1".to_string()));
    }

    #[test]
    fn test_resolved_album_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let album = make_album("album_1", "Purple Rain");
        let track1 = make_track("track_1", "When Doves Cry", "album_1");
        let track2 = make_track("track_2", "Purple Rain", "album_1");
        let image = make_image("img_1");

        let resolved = ResolvedAlbum {
            album,
            artists: vec![artist],
            discs: vec![Disc {
                number: 1,
                name: None,
                tracks: vec![track1, track2],
            }],
            display_image: Some(image),
        };

        let summary = AlbumSummary::from(&resolved);

        assert_eq!(summary.id, "album_1");
        assert_eq!(summary.name, "Purple Rain");
        assert_eq!(summary.release_year, Some(1984));
        assert_eq!(summary.track_count, 2);
        assert_eq!(summary.image_id, Some("img_1".to_string()));
        assert_eq!(summary.artist_names, vec!["Prince"]);
    }

    #[test]
    fn test_album_summary_basic() {
        let album = make_album("album_1", "Purple Rain");
        let summary = album_summary_basic(&album, vec!["Prince".to_string()]);

        assert_eq!(summary.id, "album_1");
        assert_eq!(summary.name, "Purple Rain");
        assert_eq!(summary.release_year, Some(1984));
        assert_eq!(summary.track_count, 0);
        assert!(summary.image_id.is_none());
        assert_eq!(summary.artist_names, vec!["Prince"]);
    }

    #[test]
    fn test_resolved_artist_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let image = make_image("img_1");

        let resolved = ResolvedArtist {
            artist,
            display_image: Some(image),
            related_artists: vec![],
        };

        let summary = ArtistSummary::from(&resolved);

        assert_eq!(summary.id, "artist_1");
        assert_eq!(summary.name, "Prince");
        assert_eq!(summary.image_id, Some("img_1".to_string()));
    }

    #[test]
    fn test_artist_to_summary() {
        let artist = make_artist("artist_1", "Prince");
        let summary = ArtistSummary::from(&artist);

        assert_eq!(summary.id, "artist_1");
        assert_eq!(summary.name, "Prince");
        assert!(summary.image_id.is_none());
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
