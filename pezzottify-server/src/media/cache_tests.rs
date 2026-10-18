use super::*;
use crate::backup::DbRegistry;
use crate::catalog_store::SqliteCatalogStore;
use crate::media::cache::{CacheKey, TestCache};
use crate::media::Provenance;
use futures::StreamExt;
use std::io::Read;
use std::path::PathBuf;

const JPEG: &[u8] = include_bytes!("../../tests/fixtures/test-image.jpg");
const AUDIO: &[u8] = b"0123456789abcdef";

struct CacheFixture {
    root: tempfile::TempDir,
    cache_dir: tempfile::TempDir,
    media: PathBuf,
    manager: Arc<MediaManager>,
    cache: Arc<TestCache>,
    rejected: bool,
}

impl CacheFixture {
    fn new(with_cache: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let media = root.path().join("media");
        std::fs::create_dir_all(&media).unwrap();
        let db = root.path().join("catalog.db");
        let catalog =
            Arc::new(SqliteCatalogStore::new(&db, &media, 2, &DbRegistry::new()).unwrap());
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision) VALUES ('album', 'Album', 'album', '', 0, '2026', 'year');
             INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, audio_uri, track_available) VALUES ('track1', 'Track', 1, 1, 0, 1, 1000, 0, NULL, 1);
             INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, audio_uri, track_available) VALUES ('legacy', 'Legacy', 1, 2, 0, 1, 1000, 0, 'audio/legacy.mp3', 1);",
        )
        .unwrap();
        std::fs::create_dir(media.join("audio")).unwrap();
        std::fs::write(media.join("audio/legacy.mp3"), AUDIO).unwrap();
        let manager = Arc::new(MediaManager::new(
            catalog,
            DbExecutor::new(Default::default()),
        ));
        manager.configure_search(Arc::new(crate::search::NoopSearchVault));
        let cache = Arc::new(TestCache::new());
        if with_cache {
            manager.configure_cache(cache.clone());
        }
        Self {
            root,
            cache_dir: tempfile::tempdir().unwrap(),
            media,
            manager,
            cache,
            rejected: false,
        }
    }

    fn seed_image_url(&self, url: &str) {
        rusqlite::Connection::open(self.root.path().join("catalog.db"))
            .unwrap()
            .execute(
                "INSERT INTO album_images (album_rowid, url, width, height) VALUES (1, ?1, 300, 300)",
                [url],
            )
            .unwrap();
    }

    fn cache_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.cache_dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    async fn publish(
        &self,
        id: &str,
        extension: &str,
        bytes: &[u8],
        provenance: Provenance,
    ) -> String {
        let source = self.root.path().join(format!("{id}.{extension}"));
        std::fs::write(&source, bytes).unwrap();
        self.manager
            .publish_file(id.to_owned(), extension.to_owned(), source, provenance)
            .await
            .unwrap()
            .uri
    }

    /// Replace the media root with a regular file so every original open, stat
    /// and presence probe fails, regardless of the test process user.
    fn reject_original_media(&mut self) {
        let backup = self.media.with_extension("rejected");
        std::fs::rename(&self.media, &backup).unwrap();
        std::fs::write(&self.media, b"original media storage is unavailable").unwrap();
        self.rejected = true;
    }

    fn restore_original_media(&mut self) {
        if !self.rejected {
            return;
        }
        let backup = self.media.with_extension("rejected");
        let _ = std::fs::remove_file(&self.media);
        let _ = std::fs::rename(&backup, &self.media);
        self.rejected = false;
    }
}

impl Drop for CacheFixture {
    fn drop(&mut self) {
        self.restore_original_media();
    }
}

async fn collect(mut stream: MediaStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    bytes
}

#[tokio::test]
async fn cached_audio_serves_playback_and_ranges_without_original_access() {
    let mut fixture = CacheFixture::new(true);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let path = fixture.cache_file("track1.mp3", AUDIO);
    fixture
        .cache
        .insert(CacheKey::audio("track1", &uri).unwrap(), path);
    fixture.reject_original_media();

    let (track, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .expect("track exists");
    assert_eq!(track.id, "track1");
    let audio = opened.expect("cache hit");
    assert_eq!(
        audio.metadata().await.unwrap().content_length,
        AUDIO.len() as u64
    );
    assert_eq!(
        collect(audio.range_stream(3, 4).await.unwrap()).await,
        &AUDIO[3..7]
    );

    let audio = fixture
        .manager
        .open_local_audio_blocking("track1")
        .unwrap()
        .expect("background cache hit");
    let (mut file, filename) = audio.into_reader();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, AUDIO);
    assert_eq!(filename, "track1.mp3");
    assert_eq!(fixture.cache.lookups(), 2);
}

#[tokio::test]
async fn cached_image_serves_without_original_access() {
    let mut fixture = CacheFixture::new(true);
    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/cover.jpg";
    fixture.seed_image_url(url);
    let path = fixture.cache_file("cover.jpg", JPEG);
    fixture.cache.insert(
        CacheKey::image("album", url, super::mutations::process_epoch()),
        path,
    );
    fixture.reject_original_media();

    let image = fixture.manager.read_image("album").await.unwrap();
    assert_eq!(image.bytes, JPEG);
    assert_eq!(image.content_type, "image/jpeg");
    assert_eq!(fixture.cache.lookups(), 1);
}

#[tokio::test]
async fn cache_miss_falls_back_to_authoritative_media() {
    let fixture = CacheFixture::new(true);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    assert!(uri.starts_with("audio/.managed/track1."));
    let (_, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    let audio = opened.expect("authoritative open");
    assert_eq!(
        collect(audio.range_stream(0, AUDIO.len() as u64).await.unwrap()).await,
        AUDIO
    );
    assert_eq!(fixture.cache.lookups(), 1);

    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/cover.jpg";
    fixture.seed_image_url(url);
    let image = fixture.manager.read_image("album").await.unwrap();
    assert_eq!(image.bytes, JPEG);
    assert_eq!(fixture.cache.lookups(), 2);
}

#[tokio::test]
async fn disabled_cache_leaves_authoritative_reads_unchanged() {
    let fixture = CacheFixture::new(false);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    assert!(CacheKey::audio("track1", &uri).is_some());
    let (_, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        collect(
            opened
                .unwrap()
                .range_stream(0, AUDIO.len() as u64)
                .await
                .unwrap()
        )
        .await,
        AUDIO
    );
    assert_eq!(fixture.cache.lookups(), 0);

    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let image = fixture.manager.read_image("album").await.unwrap();
    assert_eq!(image.bytes, JPEG);
    assert_eq!(fixture.cache.lookups(), 0);
}

#[tokio::test]
async fn legacy_and_unmanaged_audio_bypass_the_cache() {
    let fixture = CacheFixture::new(true);
    assert!(CacheKey::audio("legacy", "audio/legacy.mp3").is_none());
    assert!(CacheKey::audio("track1", "audio/.managed/track1.not-a-uuid.mp3").is_none());
    let (_, opened) = fixture
        .manager
        .lookup_audio("legacy")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        collect(
            opened
                .unwrap()
                .range_stream(0, AUDIO.len() as u64)
                .await
                .unwrap()
        )
        .await,
        AUDIO
    );
    assert_eq!(fixture.cache.lookups(), 0);
}

#[tokio::test]
async fn replacement_invalidates_audio_before_the_new_version_is_visible() {
    let fixture = CacheFixture::new(true);
    let first = fixture
        .publish("track1", "mp3", b"old-version", Provenance::Ingested)
        .await;
    let path = fixture.cache_file("old.mp3", b"old-version");
    fixture
        .cache
        .insert(CacheKey::audio("track1", &first).unwrap(), path);

    let second = fixture
        .publish("track1", "mp3", b"new-version", Provenance::Ingested)
        .await;
    assert_ne!(first, second);
    assert_eq!(fixture.cache.len(), 0);
    assert!(fixture
        .cache
        .invalidated()
        .iter()
        .any(|media| { media.kind == vault::MediaKind::Audio && media.id == "track1" }));

    let (_, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        collect(opened.unwrap().range_stream(0, 11).await.unwrap()).await,
        b"new-version"
    );
}

#[tokio::test]
async fn deletion_invalidates_audio_cache() {
    let fixture = CacheFixture::new(true);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let path = fixture.cache_file("track1.mp3", AUDIO);
    fixture
        .cache
        .insert(CacheKey::audio("track1", &uri).unwrap(), path);

    let copy = fixture
        .manager
        .authoritative_audio("track1")
        .unwrap()
        .expect("current copy");
    assert!(fixture.manager.delete_authoritative_audio(&copy).unwrap());
    assert_eq!(fixture.cache.len(), 0);
    assert!(fixture
        .cache
        .invalidated()
        .iter()
        .any(|media| { media.kind == vault::MediaKind::Audio && media.id == "track1" }));

    let (track, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(track.audio_uri, None);
    assert!(opened.is_none());
}

#[tokio::test]
async fn image_replacement_with_the_same_source_invalidates_the_cache() {
    let fixture = CacheFixture::new(true);
    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/cover.jpg";
    fixture.seed_image_url(url);
    let path = fixture.cache_file("cover.jpg", JPEG);
    fixture.cache.insert(
        CacheKey::image("album", url, super::mutations::process_epoch()),
        path,
    );

    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    assert_eq!(fixture.cache.len(), 0);
    assert!(fixture
        .cache
        .invalidated()
        .iter()
        .any(|media| media.kind == vault::MediaKind::Image && media.id == "album"));
}

#[tokio::test]
async fn concurrent_readers_share_cached_objects() {
    let mut fixture = CacheFixture::new(true);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let path = fixture.cache_file("track1.mp3", AUDIO);
    fixture
        .cache
        .insert(CacheKey::audio("track1", &uri).unwrap(), path);
    fixture.reject_original_media();

    let mut readers = tokio::task::JoinSet::new();
    for start in 0..8u64 {
        let manager = fixture.manager.clone();
        readers.spawn(async move {
            let (_, opened) = manager.lookup_audio("track1").await.unwrap().unwrap();
            collect(opened.unwrap().range_stream(start, 2).await.unwrap()).await
        });
    }
    while let Some(result) = readers.join_next().await {
        let bytes = result.unwrap();
        assert_eq!(bytes.len(), 2);
    }
    assert_eq!(fixture.cache.lookups(), 8);
}
