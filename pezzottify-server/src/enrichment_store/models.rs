//! Data models for the enrichment database.

use serde::{Deserialize, Serialize};

/// Audio features extracted from track analysis.
///
/// DSP features are computed from pure signal processing algorithms.
/// Classifier features (vocal_instrumental, valence) use SVM models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeatures {
    pub track_id: String,
    // -- Rhythm --
    pub bpm: f64,
    pub danceability: f64,
    // -- Tonal --
    pub key: String,              // e.g. "C major", "A minor"
    pub chords_key: String,       // overall harmonic key from chord analysis
    pub chords_scale: String,     // "major" or "minor"
    pub chords_changes_rate: f64, // harmonic complexity (chord changes per second)
    // -- Loudness --
    pub loudness: f64,            // EBU R128 integrated loudness (LUFS)
    pub average_loudness: f64,    // RMS energy (0.0-1.0)
    pub dynamic_complexity: f64,  // loudness variance over time
    // -- Timbre --
    pub spectral_complexity: f64, // timbral complexity (0.0-1.0)
    // -- Classifiers (SVM) --
    pub vocal_instrumental: f64,  // 0.0=instrumental, 1.0=vocal-heavy
    pub valence: f64,             // 0.0=sad/dark, 1.0=happy/bright
    // -- Metadata --
    pub analyzed_at: i64,
    pub analyzer_version: String,
}

/// Canonical enrichment data for an artist (populated by external agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistEnrichment {
    pub artist_id: String,
    pub entity_type: Option<String>,
    pub nationalities: Option<Vec<String>>,
    pub decades_active: Option<Vec<String>>,
    pub is_composer: Option<bool>,
    pub is_producer: Option<bool>,
    pub instruments: Option<Vec<String>>,
    pub gender: Option<String>,
    pub vocal_type: Option<String>,
    pub primary_language: Option<String>,
    pub enriched_at: i64,
    pub source: String,
}

/// Canonical enrichment data for an album (populated by external agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumEnrichment {
    pub album_id: String,
    pub is_live: Option<bool>,
    pub is_compilation: Option<bool>,
    pub is_soundtrack: Option<bool>,
    pub is_concept_album: Option<bool>,
    pub is_remix_album: Option<bool>,
    pub primary_language: Option<String>,
    pub production_era: Option<String>,
    pub enriched_at: i64,
    pub source: String,
}

/// Summary statistics for the enrichment database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStats {
    pub tracks_analyzed: usize,
    pub artists_enriched: usize,
    pub albums_enriched: usize,
}
