//! MusicBrainz API client for resolving Spotify IDs to MBIDs.
//!
//! Rate limited to 1 request per second per MusicBrainz API policy.

use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MUSICBRAINZ_API_BASE: &str = "https://musicbrainz.org/ws/2";
const RATE_LIMIT_INTERVAL: Duration = Duration::from_millis(1100); // slightly over 1s for safety

pub struct MusicBrainzClient {
    client: Client,
    last_request: Mutex<Instant>,
}

#[derive(Deserialize)]
struct UrlLookupResponse {
    #[serde(default)]
    relations: Vec<UrlRelation>,
}

#[derive(Deserialize)]
struct UrlRelation {
    artist: Option<RelatedArtist>,
}

#[derive(Deserialize)]
struct RelatedArtist {
    id: String,
}

#[derive(Deserialize)]
struct MbRelation {
    url: Option<MbUrl>,
}

#[derive(Deserialize)]
struct MbUrl {
    resource: Option<String>,
}

#[derive(Deserialize)]
struct ArtistLookupResponse {
    #[serde(default)]
    relations: Vec<MbRelation>,
}

impl MusicBrainzClient {
    pub fn new(user_agent: &str) -> Result<Self> {
        let client = Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            last_request: Mutex::new(Instant::now() - RATE_LIMIT_INTERVAL),
        })
    }

    fn rate_limit(&self) {
        let mut last = self.last_request.lock().unwrap();
        let elapsed = last.elapsed();
        if elapsed < RATE_LIMIT_INTERVAL {
            std::thread::sleep(RATE_LIMIT_INTERVAL - elapsed);
        }
        *last = Instant::now();
    }

    /// Look up a MusicBrainz artist ID by Spotify ID.
    ///
    /// Uses the `/ws/2/url` endpoint to find the artist linked to the given
    /// Spotify artist URL via `artist-rels`.
    pub fn lookup_mbid_for_spotify_id(&self, spotify_id: &str) -> Result<Option<String>> {
        self.rate_limit();

        let spotify_url = format!("https://open.spotify.com/artist/{}", spotify_id);
        let url = format!(
            "{}/url?resource={}&inc=artist-rels&fmt=json",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&spotify_url)
        );

        let response = self.client.get(&url).send()?;
        let status = response.status();

        if !status.is_success() {
            if status.as_u16() == 404 {
                // No URL entity for this Spotify link — genuinely not found
                return Ok(None);
            }
            // Transient errors (503, 429, etc.) — return Err so job retries
            anyhow::bail!("MusicBrainz URL lookup failed with status {}", status);
        }

        let body: UrlLookupResponse = response.json()?;

        for rel in &body.relations {
            if let Some(artist) = &rel.artist {
                return Ok(Some(artist.id.clone()));
            }
        }

        Ok(None)
    }

    /// Look up a Spotify artist ID from a MusicBrainz ID.
    ///
    /// Queries the artist's URL relations for a Spotify link.
    pub fn lookup_spotify_id_for_mbid(&self, mbid: &str) -> Result<Option<String>> {
        self.rate_limit();

        let url = format!(
            "{}/artist/{}?inc=url-rels&fmt=json",
            MUSICBRAINZ_API_BASE, mbid
        );

        let response = self.client.get(&url).send()?;
        let status = response.status();

        if !status.is_success() {
            if status.as_u16() == 404 {
                return Ok(None);
            }
            anyhow::bail!("MusicBrainz artist lookup failed with status {}", status);
        }

        let body: ArtistLookupResponse = response.json()?;

        for rel in &body.relations {
            if let Some(url) = &rel.url {
                if let Some(resource) = &url.resource {
                    if resource.contains("open.spotify.com/artist/") {
                        if let Some(id) = resource.rsplit('/').next() {
                            return Ok(Some(id.to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}
