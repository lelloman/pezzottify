//! Last.fm API client for fetching similar artists.
//!
//! Rate limited to 5 requests per second per Last.fm API guidelines.

use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

use super::pacing::RequestPacer;

const LASTFM_API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";
const RATE_LIMIT_INTERVAL: Duration = Duration::from_millis(200); // 5 req/sec

/// A similar artist as returned by Last.fm.
#[derive(Debug, Clone)]
pub struct SimilarArtist {
    pub name: String,
    pub mbid: Option<String>,
    pub score: f64,
}

pub struct LastFmClient {
    client: Client,
    api_key: String,
    pacer: RequestPacer,
}

#[derive(Deserialize)]
struct SimilarArtistsResponse {
    similarartists: Option<SimilarArtistsContainer>,
}

#[derive(Deserialize)]
struct SimilarArtistsContainer {
    artist: Option<Vec<LastFmArtist>>,
}

#[derive(Deserialize)]
struct LastFmArtist {
    name: Option<String>,
    mbid: Option<String>,
    #[serde(rename = "match")]
    match_score: Option<String>,
}

impl LastFmClient {
    pub fn new(api_key: &str) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            api_key: api_key.to_string(),
            pacer: RequestPacer::new(RATE_LIMIT_INTERVAL),
        })
    }

    fn rate_limit(&self) {
        self.pacer.wait();
    }

    /// Get similar artists for an artist identified by MusicBrainz ID.
    pub fn get_similar_artists(&self, mbid: &str, limit: usize) -> Result<Vec<SimilarArtist>> {
        self.rate_limit();

        let url = format!(
            "{}?method=artist.getsimilar&mbid={}&api_key={}&format=json&limit={}",
            LASTFM_API_BASE, mbid, self.api_key, limit
        );

        let response = self.client.get(&url).send()?;

        if !response.status().is_success() {
            // Transient errors (429, 5xx) — return Err so job retries
            anyhow::bail!("Last.fm API failed with status {}", response.status());
        }

        let body: SimilarArtistsResponse = response.json()?;

        let artists = body
            .similarartists
            .and_then(|sa| sa.artist)
            .unwrap_or_default();

        let results: Vec<SimilarArtist> = artists
            .into_iter()
            .filter_map(|a| {
                let name = a.name?;
                let score: f64 = a
                    .match_score
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let mbid = a.mbid.filter(|m| !m.is_empty());
                Some(SimilarArtist { name, mbid, score })
            })
            .collect();

        Ok(results)
    }
}
