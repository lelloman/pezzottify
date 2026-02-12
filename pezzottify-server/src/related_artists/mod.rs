//! Related artists enrichment via MusicBrainz and Last.fm APIs.
//!
//! This module provides API clients for resolving related artists:
//! - MusicBrainz: Maps Spotify IDs to/from MusicBrainz IDs (MBIDs)
//! - Last.fm: Fetches similar artists by MBID

pub mod lastfm;
pub mod musicbrainz;
