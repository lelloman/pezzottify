//! Shared LRCLIB acquisition for scheduled and user-requested lyrics downloads.
use crate::catalog_store::{CatalogStore, ResolvedTrack, TrackAvailability};
use crate::db_executor::{DbHandle, DbPriority};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Mutex;

// Share pacing and cache rechecks across the daily job and interactive requests.
static PROVIDER_GATE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MISS_RETRY_SECS: i64 = 30 * 24 * 60 * 60;
const ERROR_RETRY_SECS: i64 = 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackLyrics {
    pub track_id: String,
    pub provider: String,
    pub provider_id: Option<i64>,
    /// found, instrumental, not_found, or error.
    pub status: String,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub fetched_at: i64,
    pub retry_at: i64,
}

impl TrackLyrics {
    fn empty(track_id: String, status: &str, now: i64) -> Self {
        Self {
            track_id,
            provider: "lrclib".into(),
            provider_id: None,
            status: status.into(),
            plain_lyrics: None,
            synced_lyrics: None,
            fetched_at: now,
            retry_at: now + MISS_RETRY_SECS,
        }
    }
    fn cached(&self, now: i64) -> bool {
        matches!(self.status.as_str(), "found" | "instrumental") || self.retry_at > now
    }
}

#[derive(Debug, Default, Serialize)]
pub struct LyricsSummary {
    pub found: usize,
    pub instrumental: usize,
    pub not_found: usize,
    pub cached: usize,
    pub unavailable: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderLyrics {
    id: i64,
    track_name: String,
    artist_name: String,
    duration: f64,
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn matched(result: ProviderLyrics, track: &ResolvedTrack, now: i64) -> Result<TrackLyrics> {
    // A title match alone is unsafe for live recordings, edits, and covers.
    let artist = track.artists.first().context("Track has no artist")?;
    if normalize(&result.track_name) != normalize(&track.track.name)
        || normalize(&result.artist_name) != normalize(&artist.artist.name)
        || !result.duration.is_finite()
        || (result.duration - track.track.duration_ms as f64 / 1000.0).abs() > 2.0
    {
        bail!("LRCLIB returned a different recording");
    }
    let plain = result.plain_lyrics.filter(|text| !text.trim().is_empty());
    let synced = result.synced_lyrics.filter(|text| !text.trim().is_empty());
    let status = if result.instrumental {
        "instrumental"
    } else if plain.is_some() || synced.is_some() {
        "found"
    } else {
        "not_found"
    };
    let mut lyrics = TrackLyrics::empty(track.track.id.clone(), status, now);
    lyrics.provider_id = Some(result.id);
    if !result.instrumental {
        lyrics.plain_lyrics = plain;
        lyrics.synced_lyrics = synced;
    }
    Ok(lyrics)
}

pub struct LyricsFetcher {
    client: reqwest::Client,
    base_url: String,
}
impl LyricsFetcher {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("Pezzottify/lyrics (https://github.com/lelloman/pezzottify)")
                .timeout(Duration::from_secs(20))
                .build()?,
            base_url: "https://lrclib.net".into(),
        })
    }
    async fn lookup(&self, track: &ResolvedTrack) -> Result<TrackLyrics> {
        let now = chrono::Utc::now().timestamp();
        let Some(artist) = track
            .artists
            .first()
            .filter(|a| !a.artist.name.trim().is_empty())
        else {
            return Ok(TrackLyrics::empty(track.track.id.clone(), "not_found", now));
        };
        if track.track.duration_ms <= 0 || track.track.name.trim().is_empty() {
            return Ok(TrackLyrics::empty(track.track.id.clone(), "not_found", now));
        }
        let mut response = self
            .client
            .get(format!("{}/api/get", self.base_url))
            .query(&[
                ("track_name", track.track.name.clone()),
                ("artist_name", artist.artist.name.clone()),
                ("album_name", track.album.name.clone()),
                (
                    "duration",
                    (track.track.duration_ms as f64 / 1000.0).to_string(),
                ),
            ])
            .send()
            .await?;
        let now = chrono::Utc::now().timestamp();
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(TrackLyrics::empty(track.track.id.clone(), "not_found", now));
        }
        response.error_for_status_ref()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                bail!("Lyrics response too large");
            }
            bytes.extend_from_slice(&chunk);
        }
        let provider = serde_json::from_slice(&bytes)?;
        // An unsuitable recording is a miss for this track, not a provider outage.
        Ok(matched(provider, track, now)
            .unwrap_or_else(|_| TrackLyrics::empty(track.track.id.clone(), "not_found", now)))
    }

    pub async fn download(
        &self,
        db: &DbHandle<dyn CatalogStore>,
        ids: Vec<String>,
        priority: DbPriority,
        force: bool,
    ) -> Result<LyricsSummary> {
        let mut summary = LyricsSummary::default();
        for id in ids {
            let _gate = PROVIDER_GATE.lock().await;
            let lookup_id = id.clone();
            let (track, cached) = db
                .run(priority, move |store| {
                    Ok((
                        store.get_resolved_track(&lookup_id)?,
                        store.get_track_lyrics(&lookup_id)?,
                    ))
                })
                .await?;
            if cached.is_some_and(|lyrics| {
                matches!(lyrics.status.as_str(), "found" | "instrumental")
                    || (!force && lyrics.cached(chrono::Utc::now().timestamp()))
            }) {
                summary.cached += 1;
                continue;
            }
            let Some(track) =
                track.filter(|t| t.track.availability == TrackAvailability::Available)
            else {
                summary.unavailable += 1;
                continue;
            };
            // Bound provider traffic even when many users request different albums.
            tokio::time::sleep(Duration::from_millis(300)).await;
            let result = self.lookup(&track).await;
            let lyrics = match &result {
                Ok(lyrics) => lyrics.clone(),
                Err(_) => {
                    let now = chrono::Utc::now().timestamp();
                    let mut lyrics = TrackLyrics::empty(id, "error", now);
                    lyrics.retry_at = now + ERROR_RETRY_SECS;
                    lyrics
                }
            };
            match lyrics.status.as_str() {
                "found" => summary.found += 1,
                "instrumental" => summary.instrumental += 1,
                "not_found" => summary.not_found += 1,
                _ => {}
            }
            db.run(priority, move |store| store.save_track_lyrics(&lyrics))
                .await?;
            // Stop on provider/network failures; do not poison the remaining batch as misses.
            result?;
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::{ArtistRole, TrackArtist};
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn track() -> ResolvedTrack {
        ResolvedTrack {
            track: serde_json::from_value(json!({"id":"track", "name":"A Song", "album_id":"album", "disc_number":1,
                "track_number":1, "duration_ms":200_000, "explicit":false, "popularity":80, "availability":"available"})).unwrap(),
            album: serde_json::from_value(json!({"id":"album", "name":"An Album", "album_type":"album", "popularity":80})).unwrap(),
            artists: vec![TrackArtist { role: ArtistRole::MainArtist, artist: serde_json::from_value(json!({
                "id":"artist", "name":"An Artist", "genres":[], "followers_total":0, "popularity":80, "available":true
            })).unwrap() }],
        }
    }
    fn provider() -> serde_json::Value {
        json!({"id":42, "trackName":"A Song", "artistName":"An Artist", "duration":200.0,
            "instrumental":false, "plainLyrics":"Example line", "syncedLyrics":"[00:01.00]Example line"})
    }
    #[test]
    fn lyrics_reject_other_recordings_and_preserve_synced_text() {
        let result = matched(serde_json::from_value(provider()).unwrap(), &track(), 100).unwrap();
        assert_eq!(result.status, "found");
        assert_eq!(
            result.synced_lyrics.as_deref(),
            Some("[00:01.00]Example line")
        );
        for (field, value) in [
            ("trackName", json!("A Song (Live)")),
            ("artistName", json!("Cover Band")),
            ("duration", json!(210)),
        ] {
            let mut response = provider();
            response[field] = value;
            assert!(matched(serde_json::from_value(response).unwrap(), &track(), 100).is_err());
        }
        let mut response = provider();
        response["instrumental"] = json!(true);
        let result = matched(serde_json::from_value(response).unwrap(), &track(), 100).unwrap();
        assert_eq!(result.status, "instrumental");
        assert!(result.plain_lyrics.is_none());
        assert!(result.synced_lyrics.is_none());
    }
    #[tokio::test]
    async fn lyrics_http_distinguishes_missing_from_provider_errors() {
        for (status, body, expected) in [
            ("200 OK", provider().to_string(), Some("found")),
            ("404 Not Found", "{}".into(), Some("not_found")),
            ("429 Too Many Requests", "{}".into(), None),
            ("500 Internal Server Error", "{}".into(), None),
            ("200 OK", "not json".into(), None),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let n = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                assert!(request.contains("/api/get?track_name=A+Song"));
                assert!(request.contains("artist_name=An+Artist"));
                assert!(request.contains("album_name=An+Album"));
                assert!(request.contains("duration=200"));
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let mut fetcher = LyricsFetcher::new().unwrap();
            fetcher.base_url = format!("http://{address}");
            let result = fetcher.lookup(&track()).await;
            match expected {
                Some(status) => assert_eq!(result.unwrap().status, status),
                None => assert!(result.is_err()),
            }
            server.await.unwrap();
        }
    }
    #[test]
    fn lyrics_misses_expire_but_successes_are_reused() {
        let mut lyrics = TrackLyrics::empty("track".into(), "not_found", 100);
        assert!(lyrics.cached(100));
        assert!(!lyrics.cached(lyrics.retry_at));
        lyrics.status = "found".into();
        assert!(lyrics.cached(i64::MAX));
    }
    #[tokio::test]
    async fn lyrics_concurrent_downloads_fetch_once_and_persist() {
        use crate::catalog_store::SqliteCatalogStore;
        use crate::db_executor::{DbExecutor, DbExecutorConfig, DbLane};
        use std::sync::Arc;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.db");
        let store =
            SqliteCatalogStore::new(&path, temp.path(), 2, &crate::backup::DbRegistry::new())
                .unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("INSERT INTO artists (id,name,followers_total,popularity) VALUES ('artist','An Artist',0,80);
            INSERT INTO albums (id,name,album_type,label,popularity,release_date,release_date_precision) VALUES ('album','An Album','album','',80,'2026','year');
            INSERT INTO tracks (id,name,album_rowid,track_number,popularity,disc_number,duration_ms,explicit,track_available) VALUES ('track','A Song',1,1,80,1,200000,0,1);
            INSERT INTO track_artists (track_rowid,artist_rowid,role) VALUES (1,1,0);").unwrap();
        let store: Arc<dyn CatalogStore> = Arc::new(store);
        let db = DbHandle::new(
            store.clone(),
            DbExecutor::new(DbExecutorConfig::default()),
            DbLane::CatalogWrite,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            stream.read(&mut request).await.unwrap();
            let body = provider().to_string();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let mut fetcher = LyricsFetcher::new().unwrap();
        fetcher.base_url = format!("http://{address}");
        let (first, second) = tokio::join!(
            fetcher.download(&db, vec!["track".into()], DbPriority::Interactive, true),
            fetcher.download(&db, vec!["track".into()], DbPriority::Background, false),
        );
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.found + second.found, 1);
        assert_eq!(first.cached + second.cached, 1);
        let stored = store.get_track_lyrics("track").unwrap().unwrap();
        assert_eq!(stored.provider_id, Some(42));
        assert_eq!(
            stored.synced_lyrics.as_deref(),
            Some("[00:01.00]Example line")
        );
        server.await.unwrap();
    }
}
