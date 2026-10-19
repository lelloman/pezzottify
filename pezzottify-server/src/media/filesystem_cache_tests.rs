use super::cache::{CacheFillRequest, CacheKey, CacheSource, MediaCache};
use super::filesystem_cache::{FilesystemCacheSettings, FilesystemCaches};
use super::vault::{MediaKey, MediaKind};
use super::*;
use crate::backup::DbRegistry;
use crate::catalog_store::SqliteCatalogStore;
use crate::media::Provenance;
use futures::StreamExt;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Barrier,
};
use std::time::{Duration, Instant};

const JPEG: &[u8] = include_bytes!("../../tests/fixtures/test-image.jpg");
const AUDIO: &[u8] = b"0123456789abcdef";

struct CacheFixture {
    root: tempfile::TempDir,
    media: PathBuf,
    manager: Arc<MediaManager>,
    caches: Arc<FilesystemCaches>,
}

impl CacheFixture {
    fn with_limits(limits: &[u64]) -> Self {
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

        let settings = limits
            .iter()
            .enumerate()
            .map(|(index, max_bytes)| FilesystemCacheSettings {
                name: format!("cache{index}"),
                path: root.path().join(format!("cache{index}")),
                max_bytes: *max_bytes,
                lru: false,
            })
            .collect::<Vec<_>>();
        let caches = Arc::new(FilesystemCaches::new(settings, &media, std::iter::empty()).unwrap());
        manager.configure_cache(caches.clone());
        Self {
            root,
            media,
            manager,
            caches,
        }
    }

    fn restarted_manager(&self, max_bytes: u64) -> MediaManager {
        let manager = MediaManager::new(
            self.manager.catalog.clone(),
            DbExecutor::new(Default::default()),
        );
        manager
            .configure_filesystem_caches(&[crate::config::MediaCacheDirectorySettings {
                name: "cache0".into(),
                path: self.root.path().join("cache0"),
                max_bytes,
                lru: false,
            }])
            .unwrap();
        manager
    }

    fn complete_cache_bytes(&self) -> u64 {
        walkdir::WalkDir::new(self.root.path().join("cache0"))
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.metadata().unwrap().len())
            .sum()
    }

    fn fill_audio(&self, uri: &str, bytes: &[u8]) -> CacheKey {
        let key = CacheKey::audio("track1", uri).unwrap();
        self.caches
            .populate(CacheFillRequest {
                epoch: self.caches.invalidation_epoch(&key.media),
                key: key.clone(),
                filename: "track1.mp3".into(),
                content_type: "audio/mpeg".into(),
                content_length: bytes.len() as u64,
                source: CacheSource::Bytes(Arc::from(bytes)),
            })
            .unwrap();
        key
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

    fn reject_original_media(&self) {
        let backup = self.media.with_extension("rejected");
        std::fs::rename(&self.media, &backup).unwrap();
        std::fs::write(&self.media, b"original media storage is unavailable").unwrap();
    }
}

impl Drop for CacheFixture {
    fn drop(&mut self) {
        let backup = self.media.with_extension("rejected");
        if backup.exists() {
            let _ = std::fs::remove_file(&self.media);
            let _ = std::fs::rename(&backup, &self.media);
        }
    }
}

async fn collect(mut stream: MediaStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    bytes
}

async fn wait_for_entry(caches: &FilesystemCaches, key: &CacheKey) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if caches.lookup(key).unwrap().is_some() {
            return;
        }
        assert!(Instant::now() < deadline, "cache fill did not complete");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_skipped_fill(caches: &FilesystemCaches) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if caches.fills_started() > 0 && caches.pending_fills() == 0 {
            return;
        }
        assert!(Instant::now() < deadline, "cache fill did not finish");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn audio_source_read_then_cache_hit_without_original_access() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = CacheKey::audio("track1", &uri).unwrap();

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
    wait_for_entry(&fixture.caches, &key).await;

    fixture.reject_original_media();
    let (_, opened) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    let audio = opened.expect("cache hit");
    assert_eq!(
        audio.metadata().await.unwrap().content_length,
        AUDIO.len() as u64
    );
    assert_eq!(
        collect(audio.range_stream(3, 4).await.unwrap()).await,
        &AUDIO[3..7]
    );
    assert_eq!(fixture.caches.used_bytes(0), AUDIO.len() as u64);
}

#[tokio::test]
async fn image_source_read_then_cache_hit_without_original_access() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/cover.jpg";
    fixture.seed_image_url(url);
    let key = CacheKey::image("album", url, super::mutations::process_epoch());

    let image = fixture.manager.read_image("album").await.unwrap();
    assert_eq!(image.bytes, JPEG);
    wait_for_entry(&fixture.caches, &key).await;

    fixture.reject_original_media();
    let image = fixture.manager.read_image("album").await.unwrap();
    assert_eq!(image.bytes, JPEG);
    assert_eq!(image.content_type, "image/jpeg");
    assert_eq!(fixture.caches.used_bytes(0), JPEG.len() as u64);
}

#[tokio::test]
async fn full_and_oversized_caches_skip_population_without_failing_reads() {
    for limit in [AUDIO.len() as u64 - 1, 4] {
        let fixture = CacheFixture::with_limits(&[limit]);
        let uri = fixture
            .publish("track1", "mp3", AUDIO, Provenance::Ingested)
            .await;
        let key = CacheKey::audio("track1", &uri).unwrap();

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
        wait_for_skipped_fill(&fixture.caches).await;
        assert!(fixture.caches.lookup(&key).unwrap().is_none());
        assert_eq!(fixture.caches.used_bytes(0), 0);

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
    }
}

#[tokio::test]
async fn concurrent_fills_are_deduplicated() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = CacheKey::audio("track1", &uri).unwrap();
    let epoch = fixture.caches.invalidation_epoch(&key.media);
    let opens = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));

    let mut workers = Vec::new();
    for _ in 0..8 {
        let caches = fixture.caches.clone();
        let key = key.clone();
        let epoch = epoch;
        let opens = opens.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let request = CacheFillRequest {
                key,
                epoch,
                filename: "track1.mp3".to_owned(),
                content_type: "audio/mpeg".to_owned(),
                content_length: AUDIO.len() as u64,
                source: CacheSource::Open(Box::new(move || {
                    opens.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new(Cursor::new(AUDIO)))
                })),
            };
            barrier.wait();
            caches.populate(request).unwrap();
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }

    assert_eq!(opens.load(Ordering::SeqCst), 1);
    assert!(fixture.caches.lookup(&key).unwrap().is_some());
}

#[tokio::test]
async fn incomplete_fills_never_become_visible() {
    struct FailingReader(usize);
    impl Read for FailingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::other("authoritative read failed"));
            }
            let amount = self.0.min(buffer.len());
            buffer[..amount].fill(b'x');
            self.0 -= amount;
            Ok(amount)
        }
    }

    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = CacheKey::audio("track1", &uri).unwrap();
    let request = CacheFillRequest {
        key: key.clone(),
        epoch: fixture.caches.invalidation_epoch(&key.media),
        filename: "track1.mp3".to_owned(),
        content_type: "audio/mpeg".to_owned(),
        content_length: AUDIO.len() as u64,
        source: CacheSource::Open(Box::new(|| Ok(Box::new(FailingReader(8))))),
    };

    assert!(fixture.caches.populate(request).is_err());
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
    assert!(fixture.caches.entry_path(0, &key).is_none());
}

#[test]
fn invalidation_during_a_fill_prevents_publication() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    let started = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let caches = fixture.caches.clone();
    let worker_key = key.clone();
    let worker_started = started.clone();
    let worker_release = release.clone();
    let worker = std::thread::spawn(move || {
        let request = CacheFillRequest {
            key: worker_key,
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Open(Box::new(move || {
                worker_started.wait();
                worker_release.wait();
                Ok(Box::new(Cursor::new(AUDIO)))
            })),
        };
        caches.populate(request)
    });

    started.wait();
    fixture.caches.invalidate(&MediaKey {
        kind: MediaKind::Audio,
        id: "track1".to_owned(),
    });
    release.wait();
    assert!(worker.join().unwrap().is_err());
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
}

#[test]
fn a_read_that_began_before_invalidation_is_not_cached() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    fixture.caches.invalidate(&MediaKey {
        kind: MediaKind::Audio,
        id: "track1".to_owned(),
    });
    let opens = Arc::new(AtomicUsize::new(0));
    let source_opens = opens.clone();

    fixture
        .caches
        .populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Open(Box::new(move || {
                source_opens.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Cursor::new(AUDIO)))
            })),
        })
        .unwrap();

    assert_eq!(opens.load(Ordering::SeqCst), 0);
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
}

#[tokio::test]
async fn restart_keeps_durable_audio_and_drops_process_scoped_images() {
    let base = tempfile::tempdir().unwrap();
    let media = base.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    let settings = vec![FilesystemCacheSettings {
        name: "ssd".to_owned(),
        path: base.path().join("cache"),
        max_bytes: 1024 * 1024,
        lru: false,
    }];
    let audio_key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    let image_key = CacheKey::image("album", "https://example.test/cover.jpg", "first-process");

    {
        let caches = FilesystemCaches::new(settings.clone(), &media, std::iter::empty()).unwrap();
        caches
            .populate(CacheFillRequest {
                key: audio_key.clone(),
                epoch: 0,
                filename: "track1.mp3".to_owned(),
                content_type: "audio/mpeg".to_owned(),
                content_length: AUDIO.len() as u64,
                source: CacheSource::Bytes(Arc::from(AUDIO)),
            })
            .unwrap();
        caches
            .populate(CacheFillRequest {
                key: image_key.clone(),
                epoch: 0,
                filename: "album.jpg".to_owned(),
                content_type: "image/jpeg".to_owned(),
                content_length: JPEG.len() as u64,
                source: CacheSource::Bytes(Arc::from(JPEG)),
            })
            .unwrap();
        assert!(caches.lookup(&audio_key).unwrap().is_some());
        assert!(caches.lookup(&image_key).unwrap().is_some());
    }

    let restarted = FilesystemCaches::new(settings, &media, [audio_key.clone()]).unwrap();
    assert!(restarted.lookup(&audio_key).unwrap().is_some());
    assert!(restarted.lookup(&image_key).unwrap().is_none());
    assert_eq!(restarted.used_bytes(0), AUDIO.len() as u64);
}

#[tokio::test]
async fn full_audio_cache_recovers_nonzero_epoch_without_original_access() {
    let fixture = CacheFixture::with_limits(&[AUDIO.len() as u64]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = CacheKey::audio("track1", &uri).unwrap();
    fixture.caches.invalidate(&key.media);
    assert!(fixture.caches.invalidation_epoch(&key.media) > 0);
    fixture.fill_audio(&uri, AUDIO);
    assert_eq!(fixture.caches.used_bytes(0), AUDIO.len() as u64);
    assert_eq!(fixture.complete_cache_bytes(), AUDIO.len() as u64);

    // Build a new manager through production startup recovery, with every
    // original-media open/stat/presence operation rejected by the media root.
    fixture.reject_original_media();
    // Availability projections may mark original storage unavailable without
    // changing its durable revision. That must not discard a useful warm copy.
    rusqlite::Connection::open(fixture.root.path().join("catalog.db"))
        .unwrap()
        .execute("UPDATE tracks SET track_available=0 WHERE id='track1'", [])
        .unwrap();
    for _ in 0..2 {
        let manager = fixture.restarted_manager(AUDIO.len() as u64);
        let caches = manager.cache.get().unwrap();
        assert_eq!(caches.invalidation_epoch(&key.media), 0);
        assert!(caches.lookup(&key).unwrap().is_some());
        let (_, opened) = manager.lookup_audio("track1").await.unwrap().unwrap();
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
        assert_eq!(fixture.complete_cache_bytes(), AUDIO.len() as u64);
    }
    // Independently inspect the reconstructed ledger, and prove the recovered
    // full allocation rejects another admission rather than oversubscribing.
    let recovered = FilesystemCaches::new(
        vec![FilesystemCacheSettings {
            name: "cache0".into(),
            path: fixture.root.path().join("cache0"),
            max_bytes: AUDIO.len() as u64,
            lru: false,
        }],
        &fixture.media,
        [key.clone()],
    )
    .unwrap();
    assert_eq!(recovered.used_bytes(0), AUDIO.len() as u64);
    assert_eq!(recovered.reserved_bytes(0), 0);
    let another = CacheKey::audio(
        "track2",
        &format!("audio/.managed/track2.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    recovered
        .populate(CacheFillRequest {
            key: another.clone(),
            epoch: 0,
            filename: "track2.mp3".into(),
            content_type: "audio/mpeg".into(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Open(Box::new(|| {
                panic!("full recovered cache must not open source")
            })),
        })
        .unwrap();
    assert!(recovered.lookup(&another).unwrap().is_none());
    assert_eq!(recovered.used_bytes(0), AUDIO.len() as u64);
}

#[tokio::test]
async fn restart_reconciles_failed_audio_cleanup_after_replacement_and_deletion() {
    for delete in [false, true] {
        let fixture = CacheFixture::with_limits(&[AUDIO.len() as u64]);
        let old_uri = fixture
            .publish("track1", "mp3", AUDIO, Provenance::Ingested)
            .await;
        let old_key = fixture.fill_audio(&old_uri, AUDIO);
        // A file blocks mkdir(trash), so invalidation cannot remove the old
        // epoch. Recovery must use the catalog revision, not the largest epoch.
        let trash = fixture.root.path().join("cache0/trash");
        std::fs::write(&trash, []).unwrap();
        let replacement = b"fedcba9876543210";
        let new_uri = if delete {
            let copy = fixture
                .manager
                .authoritative_audio("track1")
                .unwrap()
                .unwrap();
            assert!(fixture.manager.delete_authoritative_audio(&copy).unwrap());
            None
        } else {
            Some(
                fixture
                    .publish("track1", "mp3", replacement, Provenance::Ingested)
                    .await,
            )
        };
        assert!(fixture.caches.lookup(&old_key).unwrap().is_none());
        assert_eq!(fixture.caches.used_bytes(0), AUDIO.len() as u64);
        assert_eq!(fixture.complete_cache_bytes(), AUDIO.len() as u64);
        let manager = fixture.restarted_manager(AUDIO.len() as u64);
        let caches = manager.cache.get().unwrap();
        assert!(caches.lookup(&old_key).unwrap().is_none());
        assert_eq!(fixture.complete_cache_bytes(), 0);
        assert!(!trash.exists());
        if let Some(uri) = new_uri {
            let key = CacheKey::audio("track1", &uri).unwrap();
            caches
                .populate(CacheFillRequest {
                    epoch: caches.invalidation_epoch(&key.media),
                    key: key.clone(),
                    filename: "track1.mp3".into(),
                    content_type: "audio/mpeg".into(),
                    content_length: replacement.len() as u64,
                    source: CacheSource::Bytes(Arc::from(&replacement[..])),
                })
                .unwrap();
            assert!(caches.lookup(&key).unwrap().is_some());
            assert_eq!(fixture.complete_cache_bytes(), replacement.len() as u64);
            fixture.reject_original_media();
            let manager = fixture.restarted_manager(AUDIO.len() as u64);
            let (_, opened) = manager.lookup_audio("track1").await.unwrap().unwrap();
            assert_eq!(
                collect(
                    opened
                        .unwrap()
                        .range_stream(0, replacement.len() as u64)
                        .await
                        .unwrap()
                )
                .await,
                replacement
            );
        } else {
            fixture.reject_original_media();
            assert!(manager
                .lookup_audio("track1")
                .await
                .unwrap()
                .unwrap()
                .1
                .is_none());
        }
    }
}

#[tokio::test]
async fn restart_reconciles_legacy_layout_and_discards_uncertain_audio_files() {
    let fixture = CacheFixture::with_limits(&[AUDIO.len() as u64]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = fixture.fill_audio(&uri, AUDIO);
    let path = fixture.caches.entry_path(0, &key).unwrap();
    let identity = path.parent().unwrap().parent().unwrap();
    // Original flat layout, plus unpublishable/unknown leftovers.
    std::fs::rename(&path, identity.join(path.file_name().unwrap())).unwrap();
    std::fs::write(identity.join(".tmp-interrupted"), AUDIO).unwrap();
    std::fs::write(identity.join("unknown.mp3"), AUDIO).unwrap();
    // Old payload-checksum entries are disposable, even for the current key.
    let key_hash = path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let old_checksum_entry = identity.join(format!("{key_hash}.{}.mp3", "a".repeat(64)));
    std::fs::write(&old_checksum_entry, AUDIO).unwrap();
    fixture.reject_original_media();
    let manager = fixture.restarted_manager(AUDIO.len() as u64);
    assert!(manager.cache.get().unwrap().lookup(&key).unwrap().is_some());
    assert_eq!(fixture.complete_cache_bytes(), AUDIO.len() as u64);
    assert!(!old_checksum_entry.exists());
}

#[test]
fn population_uses_the_first_cache_with_room_and_lookup_follows_order() {
    let base = tempfile::tempdir().unwrap();
    let media = base.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    let key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();

    let too_small_then_roomy = FilesystemCaches::new(
        vec![
            FilesystemCacheSettings {
                name: "small".to_owned(),
                path: base.path().join("small"),
                max_bytes: 4,
                lru: false,
            },
            FilesystemCacheSettings {
                name: "roomy".to_owned(),
                path: base.path().join("roomy"),
                max_bytes: 1024 * 1024,
                lru: false,
            },
        ],
        &media,
        std::iter::empty(),
    )
    .unwrap();
    too_small_then_roomy
        .populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Bytes(Arc::from(AUDIO)),
        })
        .unwrap();
    assert!(too_small_then_roomy.entry_path(0, &key).is_none());
    assert!(too_small_then_roomy.entry_path(1, &key).is_some());
    assert!(too_small_then_roomy.lookup(&key).unwrap().is_some());

    let both_roomy = FilesystemCaches::new(
        vec![
            FilesystemCacheSettings {
                name: "first".to_owned(),
                path: base.path().join("first"),
                max_bytes: 1024 * 1024,
                lru: false,
            },
            FilesystemCacheSettings {
                name: "second".to_owned(),
                path: base.path().join("second"),
                max_bytes: 1024 * 1024,
                lru: false,
            },
        ],
        &media,
        std::iter::empty(),
    )
    .unwrap();
    both_roomy
        .populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Bytes(Arc::from(AUDIO)),
        })
        .unwrap();
    assert!(both_roomy.entry_path(0, &key).is_some());
    assert!(both_roomy.entry_path(1, &key).is_none());
}

#[test]
fn cache_directories_cannot_overlap_media_storage_or_each_other() {
    let base = tempfile::tempdir().unwrap();
    let media = base.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    for path in [
        media.clone(),
        media.join("cache"),
        base.path().to_path_buf(),
    ] {
        let result = FilesystemCaches::new(
            vec![FilesystemCacheSettings {
                name: "bad".to_owned(),
                path,
                max_bytes: 1024,
                lru: false,
            }],
            &media,
            std::iter::empty(),
        );
        assert!(result.is_err());
    }

    let nested = base.path().join("cache");
    std::fs::create_dir_all(nested.join("inner")).unwrap();
    let result = FilesystemCaches::new(
        vec![
            FilesystemCacheSettings {
                name: "outer".to_owned(),
                path: nested.clone(),
                max_bytes: 1024,
                lru: false,
            },
            FilesystemCacheSettings {
                name: "inner".to_owned(),
                path: nested.join("inner"),
                max_bytes: 1024,
                lru: false,
            },
        ],
        &media,
        std::iter::empty(),
    );
    assert!(result.is_err());

    assert!(FilesystemCaches::new(
        vec![FilesystemCacheSettings {
            name: "ok".to_owned(),
            path: base.path().join("ok"),
            max_bytes: 1024,
            lru: false,
        }],
        &media,
        std::iter::empty()
    )
    .is_ok());
}

#[tokio::test]
async fn legacy_audio_fills_and_serves_ranges_with_original_unavailable() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::audio("legacy", "audio/legacy.mp3").unwrap();
    assert!(matches!(key.generation, cache::CacheGeneration::Process(_)));
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
    wait_for_entry(&fixture.caches, &key).await;
    fixture.reject_original_media();
    let (_, opened) = fixture
        .manager
        .lookup_audio("legacy")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        collect(opened.unwrap().range_stream(3, 4).await.unwrap()).await,
        &AUDIO[3..7]
    );
    let manager = fixture.manager.clone();
    let opened = tokio::task::spawn_blocking(move || {
        manager
            .open_local_audio_blocking("legacy")
            .unwrap()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        collect(opened.range_stream(2, 5).await.unwrap()).await,
        &AUDIO[2..7]
    );
}

#[tokio::test]
async fn legacy_audio_replacement_invalidates_old_copy_and_pending_fill() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let old = CacheKey::audio("legacy", "audio/legacy.mp3").unwrap();
    fixture.manager.lookup_audio("legacy").await.unwrap();
    wait_for_entry(&fixture.caches, &old).await;
    let old_epoch = fixture.caches.invalidation_epoch(&old.media);
    let uri = fixture
        .publish("legacy", "mp3", b"replacement", Provenance::Ingested)
        .await;
    assert!(fixture.caches.lookup(&old).unwrap().is_none());
    fixture
        .caches
        .populate(CacheFillRequest {
            key: old.clone(),
            epoch: old_epoch,
            filename: "legacy.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Bytes(Arc::from(AUDIO)),
        })
        .unwrap();
    assert!(fixture.caches.lookup(&old).unwrap().is_none());
    let (_, opened) = fixture
        .manager
        .lookup_audio("legacy")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        collect(opened.unwrap().range_stream(0, 11).await.unwrap()).await,
        b"replacement"
    );
    wait_for_entry(&fixture.caches, &CacheKey::audio("legacy", &uri).unwrap()).await;
    let current = fixture
        .manager
        .authoritative_audio("legacy")
        .unwrap()
        .unwrap();
    assert!(fixture
        .manager
        .delete_authoritative_audio(&current)
        .unwrap());
    assert!(fixture
        .manager
        .lookup_audio("legacy")
        .await
        .unwrap()
        .unwrap()
        .1
        .is_none());
}

#[tokio::test]
async fn legacy_audio_restart_discards_old_bytes_and_refills_existing_path() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::audio("legacy", "audio/legacy.mp3").unwrap();
    fixture.manager.lookup_audio("legacy").await.unwrap();
    wait_for_entry(&fixture.caches, &key).await;
    // Simulate a same-length external replacement while the server is stopped.
    std::fs::write(fixture.media.join("audio/legacy.mp3"), b"fedcba9876543210").unwrap();
    let restarted = fixture.restarted_manager(1024 * 1024);
    assert!(restarted
        .cache
        .get()
        .unwrap()
        .lookup(&key)
        .unwrap()
        .is_none());
    let (_, opened) = restarted.lookup_audio("legacy").await.unwrap().unwrap();
    assert_eq!(
        collect(opened.unwrap().range_stream(0, 16).await.unwrap()).await,
        b"fedcba9876543210"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while restarted
        .cache
        .get()
        .unwrap()
        .lookup(&key)
        .unwrap()
        .is_none()
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    fixture.reject_original_media();
    let (_, opened) = restarted.lookup_audio("legacy").await.unwrap().unwrap();
    assert_eq!(
        collect(opened.unwrap().range_stream(0, 16).await.unwrap()).await,
        b"fedcba9876543210"
    );
}

#[test]
fn failed_cleanup_does_not_keep_stale_entries_reachable() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::image("album", "https://example.test/cover.jpg", "first-process");
    fixture
        .caches
        .populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "album.jpg".to_owned(),
            content_type: "image/jpeg".to_owned(),
            content_length: JPEG.len() as u64,
            source: CacheSource::Bytes(Arc::from(JPEG)),
        })
        .unwrap();
    assert!(fixture.caches.lookup(&key).unwrap().is_some());

    // Force invalidation cleanup to fail: the trash directory cannot be
    // prepared because a regular file occupies its path.
    std::fs::write(fixture.root.path().join("cache0/trash"), b"not a directory").unwrap();
    fixture.caches.invalidate(&key.media);

    assert!(
        fixture.caches.lookup(&key).unwrap().is_none(),
        "an old entry must be unreachable even when cleanup fails"
    );
    assert!(fixture.caches.entry_path(0, &key).is_none());
}

#[test]
fn slow_fill_does_not_block_lookup_or_invalidation_epoch() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    let started = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let caches = fixture.caches.clone();
    let worker_key = key.clone();
    let worker_started = started.clone();
    let worker_release = release.clone();
    let worker = std::thread::spawn(move || {
        caches.populate(CacheFillRequest {
            key: worker_key,
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Open(Box::new(move || {
                worker_started.wait();
                worker_release.wait();
                Ok(Box::new(Cursor::new(AUDIO)))
            })),
        })
    });

    started.wait();
    let begin = Instant::now();
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
    assert_eq!(fixture.caches.invalidation_epoch(&key.media), 0);
    assert!(
        begin.elapsed() < Duration::from_millis(500),
        "foreground cache queries waited on a stalled fill"
    );
    release.wait();
    assert!(worker.join().unwrap().is_ok());
    assert!(fixture.caches.lookup(&key).unwrap().is_some());

    fixture.caches.invalidate(&key.media);
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
    assert_eq!(fixture.caches.used_bytes(0), 0);
}

#[test]
fn panicked_fill_releases_reservation_dedup_and_temporary_file() {
    struct PanickingReader;
    impl Read for PanickingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            buffer.fill(b'x');
            panic!("source read panicked");
        }
    }

    let limit = (AUDIO.len() * 2) as u64;
    let fixture = CacheFixture::with_limits(&[limit]);
    let key = CacheKey::audio(
        "track1",
        &format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fixture.caches.populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Open(Box::new(|| Ok(Box::new(PanickingReader)))),
        })
    }));
    assert!(result.is_err());
    assert_eq!(fixture.caches.pending_fills(), 0);
    assert_eq!(fixture.caches.reserved_bytes(0), 0);
    assert!(fixture.caches.entry_path(0, &key).is_none());

    let temporary = walkdir::WalkDir::new(fixture.root.path().join("cache0/objects"))
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp-"))
        .count();
    assert_eq!(temporary, 0, "panicked fill left a temporary file");

    fixture
        .caches
        .populate(CacheFillRequest {
            key: key.clone(),
            epoch: 0,
            filename: "track1.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Bytes(Arc::from(AUDIO)),
        })
        .unwrap();
    assert!(fixture.caches.lookup(&key).unwrap().is_some());

    let other = CacheKey::audio(
        "track2",
        &format!("audio/.managed/track2.{}.mp3", uuid::Uuid::new_v4()),
    )
    .unwrap();
    fixture
        .caches
        .populate(CacheFillRequest {
            key: other.clone(),
            epoch: 0,
            filename: "track2.mp3".to_owned(),
            content_type: "audio/mpeg".to_owned(),
            content_length: AUDIO.len() as u64,
            source: CacheSource::Bytes(Arc::from(AUDIO)),
        })
        .unwrap();
    assert!(fixture.caches.lookup(&other).unwrap().is_some());
    assert_eq!(fixture.caches.used_bytes(0), limit);
}

#[test]
fn cache_fill_admission_bounds_pending_keys_and_retained_bytes() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let key = |id: &str| {
        CacheKey::audio(
            id,
            &format!("audio/.managed/{id}.{}.mp3", uuid::Uuid::new_v4()),
        )
        .unwrap()
    };

    let keys = (0..CACHE_FILL_MAX_PENDING)
        .map(|index| key(&format!("track{index}")))
        .collect::<Vec<_>>();
    let mut permits = keys
        .iter()
        .map(|key| {
            fixture
                .manager
                .cache_fill_admission
                .admit(key, 0)
                .expect("pending slot")
        })
        .collect::<Vec<_>>();

    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&keys[0], 0)
        .is_none());
    let extra = key("extra");
    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&extra, 0)
        .is_none());
    permits.pop();
    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&extra, 0)
        .is_some());
    drop(permits);

    let first = key("image-one");
    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&first, CACHE_FILL_MAX_RETAINED_BYTES + 1)
        .is_none());
    let half = CACHE_FILL_MAX_RETAINED_BYTES / 2 + 1;
    let held = fixture
        .manager
        .cache_fill_admission
        .admit(&first, half)
        .expect("half budget");
    let second = key("image-two");
    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&second, half)
        .is_none());
    drop(held);
    assert!(fixture
        .manager
        .cache_fill_admission
        .admit(&second, half)
        .is_some());
}

#[tokio::test]
async fn timed_out_fill_retains_payload_budget_and_dedup_until_worker_finishes() {
    let fixture = CacheFixture::with_limits(&[CACHE_FILL_MAX_RETAINED_BYTES]);
    let key = CacheKey::image("one", "https://example.test/one.jpg", "process");
    let other = CacheKey::image("two", "https://example.test/two.jpg", "process");
    let admission = fixture.manager.cache_fill_admission.clone();
    let permit = admission
        .admit(&key, CACHE_FILL_MAX_RETAINED_BYTES)
        .unwrap();
    let payload: Arc<[u8]> = vec![0; CACHE_FILL_MAX_RETAINED_BYTES as usize].into();
    let weak_payload = Arc::downgrade(&payload);
    let (started, worker_started) = tokio::sync::oneshot::channel();
    let (release, worker_release) = std::sync::mpsc::channel();
    let request = CacheFillRequest {
        key: key.clone(),
        epoch: 0,
        filename: "one.jpg".into(),
        content_type: "image/jpeg".into(),
        content_length: payload.len() as u64,
        source: CacheSource::Open(Box::new(move || {
            let _ = started.send(());
            // Bound the stall so a failed assertion cannot hang runtime exit.
            worker_release.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(Box::new(Cursor::new(payload)))
        })),
    };
    let pool =
        FilesystemWorkPool::with_limits(1, Duration::from_secs(1), Duration::from_millis(50));
    let caches = fixture.caches.clone();
    let worker =
        tokio::spawn(async move { pool.run(cache_fill_work(caches, request, permit)).await });
    worker_started.await.unwrap();
    let error = worker.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("execution timed out"));
    assert!(weak_payload.upgrade().is_some());
    assert!(
        admission.admit(&key, 0).is_none(),
        "pending key released early"
    );
    assert!(
        admission.admit(&other, 1).is_none(),
        "payload budget released early"
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if admission.lock().pending.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(weak_payload.upgrade().is_none());
    assert!(admission
        .admit(&key, CACHE_FILL_MAX_RETAINED_BYTES)
        .is_some());
}

#[cfg(unix)]
#[test]
fn startup_recovery_rejects_objects_symlink_into_original_media() {
    let root = tempfile::tempdir().unwrap();
    let media = root.path().join("media");
    std::fs::create_dir_all(media.join("audio/.managed")).unwrap();
    let original = media.join("audio/.managed/original.mp3");
    std::fs::write(&original, AUDIO).unwrap();
    let cache = root.path().join("cache");
    std::fs::create_dir(&cache).unwrap();
    std::os::unix::fs::symlink(&media, cache.join("objects")).unwrap();
    let result = FilesystemCaches::new(
        vec![FilesystemCacheSettings {
            name: "ssd".into(),
            path: cache,
            max_bytes: 1024,
            lru: false,
        }],
        &media,
        std::iter::empty(),
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(original).unwrap(), AUDIO);
}

/// Reproducible workload, not a physical-HDD benchmark. Run with --nocapture.
#[tokio::test]
async fn representative_audio_seek_artwork_workload() {
    let fixture = CacheFixture::with_limits(&[16 * 1024 * 1024]);
    let audio = vec![42u8; 4 * 1024 * 1024];
    let uri = "audio/legacy.mp3";
    std::fs::write(fixture.media.join(uri), &audio).unwrap();
    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/workload.jpg";
    fixture.seed_image_url(url);
    let key = CacheKey::audio("legacy", uri).unwrap();
    let image_key = CacheKey::image("album", url, super::mutations::process_epoch());
    std::fs::write(fixture.root.path().join(".cache-cold-start"), []).unwrap();
    let cold = Instant::now();
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
                .range_stream(0, audio.len() as u64)
                .await
                .unwrap()
        )
        .await,
        audio
    );
    let cold_audio = cold.elapsed();
    // A separate cache-disabled manager keeps every baseline seek cold even
    // when the background fill triggered above finishes during the batch.
    let uncached = MediaManager::new(
        fixture.manager.catalog.clone(),
        DbExecutor::new(Default::default()),
    );
    let seeking = Instant::now();
    for index in 0..16 {
        let (_, opened) = uncached.lookup_audio("legacy").await.unwrap().unwrap();
        assert_eq!(
            collect(
                opened
                    .unwrap()
                    .range_stream(index * 4096, 4096)
                    .await
                    .unwrap()
            )
            .await,
            vec![42; 4096]
        );
    }
    let cold_seeks = seeking.elapsed();
    let cold = Instant::now();
    assert_eq!(
        fixture.manager.read_image("album").await.unwrap().bytes,
        JPEG
    );
    let cold_image = cold.elapsed();
    wait_for_entry(&fixture.caches, &key).await;
    wait_for_entry(&fixture.caches, &image_key).await;
    while fixture.caches.pending_fills() != 0 {
        tokio::task::yield_now().await;
    }

    // Includes open, stat, existence and pointer resolution: the entire original
    // root is replaced by a regular file before all warm payload/metadata calls.
    std::fs::write(fixture.root.path().join(".cache-cold-end"), []).unwrap();
    fixture.reject_original_media();
    std::fs::write(fixture.root.path().join(".cache-warm-start"), []).unwrap();
    let warm = Instant::now();
    let (_, opened) = fixture
        .manager
        .lookup_audio("legacy")
        .await
        .unwrap()
        .unwrap();
    let opened = opened.unwrap();
    assert_eq!(
        opened.metadata().await.unwrap().content_length,
        audio.len() as u64
    );
    assert_eq!(
        collect(opened.range_stream(0, audio.len() as u64).await.unwrap()).await,
        audio
    );
    let warm_audio = warm.elapsed();
    let seeking = Instant::now();
    for index in 0..16 {
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
                    .range_stream(index * 4096, 4096)
                    .await
                    .unwrap()
            )
            .await,
            vec![42; 4096]
        );
    }
    let seek_time = seeking.elapsed();
    let warm = Instant::now();
    for _ in 0..16 {
        assert_eq!(
            fixture.manager.read_image("album").await.unwrap().bytes,
            JPEG
        );
    }
    let warm_image = warm.elapsed();
    std::fs::write(fixture.root.path().join(".cache-warm-end"), []).unwrap();
    eprintln!("workload: 4MiB audio cold={cold_audio:?} warm={warm_audio:?}; 16 seeks cold={cold_seeks:?} warm={seek_time:?}; artwork {}B cold={cold_image:?} warm16={:?}; warm root rejected; use scripts/verify-media-cache.sh to audit original-storage syscalls", JPEG.len(), warm_image);
}

#[tokio::test]
async fn audio_cache_truncation_and_loss_fall_back_to_original() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    let uri = fixture
        .publish("track1", "mp3", AUDIO, Provenance::Ingested)
        .await;
    let key = fixture.fill_audio(&uri, AUDIO);
    let path = fixture.caches.entry_path(0, &key).unwrap();
    std::fs::write(&path, vec![b'X'; AUDIO.len() - 1]).unwrap();
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
    wait_for_entry(&fixture.caches, &key).await;
    std::fs::remove_file(fixture.caches.entry_path(0, &key).unwrap()).unwrap();
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
}

#[tokio::test]
async fn artwork_cache_truncation_falls_back_to_original() {
    let fixture = CacheFixture::with_limits(&[1024 * 1024]);
    fixture
        .publish("album", "jpg", JPEG, Provenance::ImageCache)
        .await;
    let url = "https://example.test/corruption.jpg";
    fixture.seed_image_url(url);
    let key = CacheKey::image("album", url, super::mutations::process_epoch());
    fixture.manager.read_image("album").await.unwrap();
    wait_for_entry(&fixture.caches, &key).await;
    let path = fixture.caches.entry_path(0, &key).unwrap();
    std::fs::write(path, vec![0; JPEG.len() - 1]).unwrap();
    assert_eq!(
        fixture.manager.read_image("album").await.unwrap().bytes,
        JPEG
    );
    assert!(fixture.caches.lookup(&key).unwrap().is_none());
}

#[test]
fn admin_snapshot_reads_accounting_without_accessing_original_or_cache_files() {
    let fixture = CacheFixture::with_limits(&[1024]);
    let uri = format!("audio/.managed/track1.{}.mp3", uuid::Uuid::new_v4());
    fixture.fill_audio(&uri, AUDIO);
    fixture.reject_original_media();
    // Snapshot must not depend even on the cache directory being accessible.
    std::fs::rename(
        fixture.root.path().join("cache0"),
        fixture.root.path().join("cache-hidden"),
    )
    .unwrap();
    let snapshots = fixture.caches.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].allocated_bytes, AUDIO.len() as u64);
    assert_eq!(snapshots[0].reserved_bytes, 0);
    assert_eq!(snapshots[0].max_bytes, 1024);
    assert!(!snapshots[0].lru);
}
