use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub enum Format {
    OGG_VORBIS_96 = 0,
    OGG_VORBIS_160 = 1,
    OGG_VORBIS_320 = 2,
    MP3_256 = 3,
    MP3_320 = 4,
    MP3_160 = 5,
    MP3_96 = 6,
    MP3_160_ENC = 7,
    AAC_24 = 8,
    AAC_48 = 9,
    FLAC_FLAC = 16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ArtistRole {
    ARTIST_ROLE_UNKNOWN = 0,
    ARTIST_ROLE_MAIN_ARTIST = 1,
    ARTIST_ROLE_FEATURED_ARTIST = 2,
    ARTIST_ROLE_REMIXER = 3,
    ARTIST_ROLE_ACTOR = 4,
    ARTIST_ROLE_COMPOSER = 5,
    ARTIST_ROLE_CONDUCTOR = 6,
    ARTIST_ROLE_ORCHESTRA = 7,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ArtistWithRole {
    pub artist_id: String,
    pub name: String,
    pub role: ArtistRole,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub album_id: String,
    pub artists_ids: Vec<String>,
    pub number: i32,
    pub disc_number: i32,
    pub duration: i32,
    pub is_explicit: bool,
    pub files: HashMap<Format, String>,
    pub alternatives: Vec<String>,
    pub tags: Vec<String>,
    pub earliest_live_timestamp: i64,
    pub has_lyrics: bool,
    pub language_of_performance: Vec<String>,
    pub original_title: String,
    pub version_title: String,
    pub artists_with_role: Vec<ArtistWithRole>,
}
