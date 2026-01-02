//! Search section types for streaming structured search.
//!
//! Each section represents a distinct part of the streaming search response.

use serde::Serialize;

use crate::search::ResolvedSearchResult;

/// The type of content that was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Artist,
    Album,
    Track,
}

/// A section of the streaming search response.
///
/// Sections are emitted progressively via SSE, allowing the client to display
/// results as they become available.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "section", rename_all = "snake_case")]
pub enum SearchSection {
    /// High-confidence single match - "This is what you wanted"
    PrimaryMatch {
        match_type: MatchType,
        item: ResolvedSearchResult,
        confidence: f64,
    },

    /// No clear winner - here are the top candidates
    TopResults { items: Vec<ResolvedSearchResult> },

    /// Popular tracks by the target artist
    PopularBy {
        target_id: String,
        target_type: MatchType,
        items: Vec<TrackSummary>,
    },

    /// Albums by the target artist
    AlbumsBy {
        target_id: String,
        items: Vec<AlbumSummary>,
    },

    /// Tracks from the target album
    TracksFrom {
        target_id: String,
        items: Vec<TrackSummary>,
    },

    /// Related artists (from artist metadata)
    RelatedArtists {
        target_id: String,
        items: Vec<ArtistSummary>,
    },

    /// Lower-relevance matches
    OtherResults { items: Vec<ResolvedSearchResult> },

    /// Stream complete
    Done { total_time_ms: u64 },
}

/// Summary of a track for enrichment sections.
#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub id: String,
    pub name: String,
    pub duration_ms: u64,
    pub track_number: Option<u32>,
    pub album_id: String,
    pub album_name: String,
    pub artist_names: Vec<String>,
    pub image_id: Option<String>,
}

/// Summary of an album for enrichment sections.
#[derive(Debug, Clone, Serialize)]
pub struct AlbumSummary {
    pub id: String,
    pub name: String,
    pub release_year: Option<i32>,
    pub track_count: u32,
    pub image_id: Option<String>,
    pub artist_names: Vec<String>,
}

/// Summary of an artist for enrichment sections.
#[derive(Debug, Clone, Serialize)]
pub struct ArtistSummary {
    pub id: String,
    pub name: String,
    pub image_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchedArtist;

    #[test]
    fn test_match_type_serialization() {
        assert_eq!(
            serde_json::to_string(&MatchType::Artist).unwrap(),
            "\"artist\""
        );
        assert_eq!(
            serde_json::to_string(&MatchType::Album).unwrap(),
            "\"album\""
        );
        assert_eq!(
            serde_json::to_string(&MatchType::Track).unwrap(),
            "\"track\""
        );
    }

    #[test]
    fn test_primary_match_serialization() {
        let section = SearchSection::PrimaryMatch {
            match_type: MatchType::Artist,
            item: ResolvedSearchResult::Artist(SearchedArtist {
                id: "artist_123".to_string(),
                name: "Prince".to_string(),
                image_id: Some("img_456".to_string()),
            }),
            confidence: 0.95,
        };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"primary_match\""));
        assert!(json.contains("\"match_type\":\"artist\""));
        assert!(json.contains("\"confidence\":0.95"));
        assert!(json.contains("\"name\":\"Prince\""));
    }

    #[test]
    fn test_top_results_serialization() {
        let section = SearchSection::TopResults { items: vec![] };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"top_results\""));
        assert!(json.contains("\"items\":[]"));
    }

    #[test]
    fn test_popular_by_serialization() {
        let section = SearchSection::PopularBy {
            target_id: "artist_123".to_string(),
            target_type: MatchType::Artist,
            items: vec![TrackSummary {
                id: "track_1".to_string(),
                name: "Purple Rain".to_string(),
                duration_ms: 240000,
                track_number: Some(1),
                album_id: "album_1".to_string(),
                album_name: "Purple Rain".to_string(),
                artist_names: vec!["Prince".to_string()],
                image_id: Some("img_1".to_string()),
            }],
        };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"popular_by\""));
        assert!(json.contains("\"target_id\":\"artist_123\""));
        assert!(json.contains("\"name\":\"Purple Rain\""));
    }

    #[test]
    fn test_albums_by_serialization() {
        let section = SearchSection::AlbumsBy {
            target_id: "artist_123".to_string(),
            items: vec![AlbumSummary {
                id: "album_1".to_string(),
                name: "Purple Rain".to_string(),
                release_year: Some(1984),
                track_count: 9,
                image_id: Some("img_1".to_string()),
                artist_names: vec!["Prince".to_string()],
            }],
        };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"albums_by\""));
        assert!(json.contains("\"release_year\":1984"));
    }

    #[test]
    fn test_related_artists_serialization() {
        let section = SearchSection::RelatedArtists {
            target_id: "artist_123".to_string(),
            items: vec![ArtistSummary {
                id: "artist_456".to_string(),
                name: "The Time".to_string(),
                image_id: None,
            }],
        };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"related_artists\""));
        assert!(json.contains("\"name\":\"The Time\""));
    }

    #[test]
    fn test_done_serialization() {
        let section = SearchSection::Done { total_time_ms: 42 };

        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("\"section\":\"done\""));
        assert!(json.contains("\"total_time_ms\":42"));
    }

    #[test]
    fn test_track_summary_serialization() {
        let summary = TrackSummary {
            id: "track_1".to_string(),
            name: "Purple Rain".to_string(),
            duration_ms: 240000,
            track_number: Some(1),
            album_id: "album_1".to_string(),
            album_name: "Purple Rain".to_string(),
            artist_names: vec!["Prince".to_string()],
            image_id: Some("img_1".to_string()),
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"duration_ms\":240000"));
        assert!(json.contains("\"track_number\":1"));
    }

    #[test]
    fn test_album_summary_with_no_year() {
        let summary = AlbumSummary {
            id: "album_1".to_string(),
            name: "Unknown Album".to_string(),
            release_year: None,
            track_count: 10,
            image_id: None,
            artist_names: vec![],
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"release_year\":null"));
        assert!(json.contains("\"image_id\":null"));
    }
}
