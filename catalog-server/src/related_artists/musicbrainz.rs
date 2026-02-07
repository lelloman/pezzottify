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
struct ArtistSearchResponse {
    artists: Option<Vec<MbArtist>>,
}

#[derive(Deserialize)]
struct MbArtist {
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
    /// Queries MusicBrainz for artists with a Spotify URL relation matching
    /// the given Spotify artist ID.
    pub fn lookup_mbid_for_spotify_id(&self, spotify_id: &str) -> Result<Option<String>> {
        self.rate_limit();

        let spotify_url = format!("https://open.spotify.com/artist/{}", spotify_id);
        let url = format!(
            "{}/artist/?query=url:\"{}\"&fmt=json&limit=1",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&spotify_url)
        );

        let response = self.client.get(&url).send()?;

        if !response.status().is_success() {
            if response.status().as_u16() == 503 {
                // Rate limited - return None to retry later
                return Ok(None);
            }
            anyhow::bail!(
                "MusicBrainz search failed with status {}",
                response.status()
            );
        }

        let body: ArtistSearchResponse = response.json()?;

        if let Some(artists) = body.artists {
            if let Some(artist) = artists.into_iter().next() {
                // Verify this artist actually has a Spotify URL relation matching our ID
                // by doing a follow-up lookup with url-rels
                return self.verify_spotify_relation(&artist.id, spotify_id);
            }
        }

        Ok(None)
    }

    /// Verify that a MusicBrainz artist has a Spotify URL relation for the given ID.
    fn verify_spotify_relation(
        &self,
        mbid: &str,
        spotify_id: &str,
    ) -> Result<Option<String>> {
        self.rate_limit();

        let url = format!(
            "{}/artist/{}?inc=url-rels&fmt=json",
            MUSICBRAINZ_API_BASE, mbid
        );

        let response = self.client.get(&url).send()?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let body: ArtistLookupResponse = response.json()?;

        let spotify_suffix = format!("/artist/{}", spotify_id);
        for rel in &body.relations {
            if let Some(url) = &rel.url {
                if let Some(resource) = &url.resource {
                    if resource.contains("spotify.com") && resource.ends_with(&spotify_suffix) {
                        return Ok(Some(mbid.to_string()));
                    }
                }
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

        if !response.status().is_success() {
            return Ok(None);
        }

        let body: ArtistLookupResponse = response.json()?;

        for rel in &body.relations {
            if let Some(url) = &rel.url {
                if let Some(resource) = &url.resource {
                    if resource.contains("open.spotify.com/artist/") {
                        // Extract the Spotify ID from the URL
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
