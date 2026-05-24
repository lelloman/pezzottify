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
    pub loudness: f64,           // EBU R128 integrated loudness (LUFS)
    pub average_loudness: f64,   // RMS energy (0.0-1.0)
    pub dynamic_complexity: f64, // loudness variance over time
    // -- Timbre --
    pub spectral_complexity: f64, // timbral complexity (0.0-1.0)
    // -- Classifiers (SVM) --
    pub vocal_instrumental: f64, // 0.0=instrumental, 1.0=vocal-heavy
    pub valence: f64,            // 0.0=sad/dark, 1.0=happy/bright
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

/// Canonical v1 enrichment data for an artist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtistEnrichmentV1 {
    pub artist_id: String,
    pub kind: Option<String>,
    pub birth_date: Option<String>,
    pub death_date: Option<String>,
    pub foundation_date: Option<String>,
    pub dissolution_date: Option<String>,
    pub origin_place: Option<String>,
    pub origin_country: Option<String>,
    pub primary_language: Option<String>,
    pub is_person: Option<bool>,
    pub is_group: Option<bool>,
    pub is_composer: Option<bool>,
    pub is_performer: Option<bool>,
    pub is_conductor: Option<bool>,
    pub is_producer: Option<bool>,
    pub confidence: Option<f64>,
    pub summary: Option<String>,
    pub bio: Option<String>,
    pub enriched_at: i64,
    pub last_verified_at: Option<i64>,
    pub source_status: Option<String>,
}

/// Canonical v1 enrichment data for an album.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlbumEnrichmentV1 {
    pub album_id: String,
    pub album_kind: Option<String>,
    pub original_release_date: Option<String>,
    pub recording_start_date: Option<String>,
    pub recording_end_date: Option<String>,
    pub release_country: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub is_live: Option<bool>,
    pub is_compilation: Option<bool>,
    pub is_soundtrack: Option<bool>,
    pub is_concept_album: Option<bool>,
    pub is_remix_album: Option<bool>,
    pub is_archival: Option<bool>,
    pub confidence: Option<f64>,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub enriched_at: i64,
    pub last_verified_at: Option<i64>,
    pub source_status: Option<String>,
}

/// Canonical v1 enrichment data for a track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackEnrichmentV1 {
    pub track_id: String,
    pub track_kind: Option<String>,
    pub work_title: Option<String>,
    pub composition_date: Option<String>,
    pub recording_date: Option<String>,
    pub language: Option<String>,
    pub is_instrumental: Option<bool>,
    pub is_live: Option<bool>,
    pub is_cover: Option<bool>,
    pub is_remix: Option<bool>,
    pub is_remaster: Option<bool>,
    pub is_arrangement: Option<bool>,
    pub movement_number: Option<i64>,
    pub movement_title: Option<String>,
    pub key_signature: Option<String>,
    pub opus_number: Option<String>,
    pub catalog_number: Option<String>,
    pub form: Option<String>,
    pub confidence: Option<f64>,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub performance_context: Option<String>,
    pub enriched_at: i64,
    pub last_verified_at: Option<i64>,
    pub source_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichmentQueueItemV1 {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub priority: i64,
    pub reason: Option<String>,
    pub stage: Option<String>,
    pub attempts: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_attempt_at: Option<i64>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityTagV1 {
    pub tag_type: String,
    pub tag: String,
    pub confidence: Option<f64>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityContributorV1 {
    pub contributor_name: String,
    pub contributor_id: Option<String>,
    pub role: String,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityRelationV1 {
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub relation_type: String,
    pub target_entity_type: Option<String>,
    pub target_entity_id: Option<String>,
    pub external_target_name: Option<String>,
    pub external_target_url: Option<String>,
    pub confidence: Option<f64>,
    pub visible: bool,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySourceV1 {
    pub source_name: String,
    pub source_url: Option<String>,
    pub retrieved_at: Option<i64>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityAliasV1 {
    pub alias: String,
    pub locale: Option<String>,
    pub source: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityExternalIdV1 {
    pub provider: String,
    pub external_id: Option<String>,
    pub url: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityEvidenceV1 {
    pub source_name: Option<String>,
    pub source_url: Option<String>,
    pub snippet: Option<String>,
    pub raw_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityEnrichmentStatusV1 {
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub stage: Option<String>,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub updated_at: Option<i64>,
    pub enriched_at: Option<i64>,
    pub source_status: Option<String>,
}

/// Summary statistics for the enrichment database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStats {
    pub tracks_analyzed: usize,
    pub artists_enriched: usize,
    pub albums_enriched: usize,
}
