use super::*;
use crate::backup::DbRegistry;
use crate::catalog_store::SqliteCatalogStore;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};

const JPEG: &[u8] = include_bytes!("../../tests/fixtures/test-image.jpg");

struct Fixture {
    manager: Arc<MediaManager>,
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("catalog.db");
        let catalog =
            Arc::new(SqliteCatalogStore::new(&db, root.path(), 2, &DbRegistry::new()).unwrap());
        let conn = rusqlite::Connection::open(db).unwrap();
        conn.execute_batch("INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision) VALUES ('album', 'Album', 'album', '', 0, '2026', 'year');
            INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, audio_uri, track_available) VALUES ('track1', 'Track', 1, 1, 0, 1, 1000, 0, 'audio/track.mp3', 1);").unwrap();
        std::fs::create_dir(root.path().join("audio")).unwrap();
        std::fs::write(root.path().join("audio/track.mp3"), b"0123456789").unwrap();
        let manager = Arc::new(MediaManager::new(
            catalog,
            DbExecutor::new(Default::default()),
        ));
        Self { manager, root }
    }

    fn image_url(&self, url: &str) {
        rusqlite::Connection::open(self.root.path().join("catalog.db")).unwrap()
            .execute("INSERT INTO album_images (album_rowid, url, width, height) VALUES (1, ?1, 300, 300)", [url]).unwrap();
    }
}

async fn upstream(
    bytes: &'static [u8],
    status: axum::http::StatusCode,
    block_cache_path: Option<PathBuf>,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = axum::Router::new().route(
        "/image",
        axum::routing::get(move || {
            let count = count.clone();
            let block_cache_path = block_cache_path.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                // Deterministically make rename fail after the initial cache miss.
                if let Some(path) = block_cache_path {
                    std::fs::create_dir_all(path).unwrap();
                }
                (status, bytes)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/image", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (url, calls, task)
}

#[tokio::test]
async fn image_cache_miss_fetches_validates_and_persists_then_hits_locally() {
    let fixture = Fixture::new();
    let (url, calls, task) = upstream(JPEG, axum::http::StatusCode::OK, None).await;
    fixture.image_url(&url);
    for _ in 0..2 {
        let image = fixture.manager.read_image("album").await.unwrap();
        assert_eq!(image.bytes, JPEG);
        assert_eq!(image.content_type, "image/jpeg");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(fixture.root.path().join("images/album.jpg")).unwrap(),
        JPEG
    );
    task.abort();
}

#[tokio::test]
async fn invalid_local_image_does_not_fall_back_to_origin() {
    let fixture = Fixture::new();
    let (url, calls, task) = upstream(JPEG, axum::http::StatusCode::OK, None).await;
    fixture.image_url(&url);
    std::fs::create_dir(fixture.root.path().join("images")).unwrap();
    std::fs::write(
        fixture.root.path().join("images/album.jpg"),
        b"not an image",
    )
    .unwrap();
    assert!(matches!(
        fixture.manager.read_image("album").await,
        Err(MediaReadError::InvalidLocalImage)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    task.abort();
}

#[tokio::test]
async fn upstream_errors_and_invalid_images_are_not_cached() {
    for (bytes, status) in [
        (b"not an image".as_slice(), axum::http::StatusCode::OK),
        (JPEG, axum::http::StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let fixture = Fixture::new();
        let (url, _, task) = upstream(bytes, status, None).await;
        fixture.image_url(&url);
        assert!(matches!(
            fixture.manager.read_image("album").await,
            Err(MediaReadError::Upstream)
        ));
        assert!(!fixture.root.path().join("images/album.jpg").exists());
        task.abort();
    }
}

#[tokio::test]
async fn image_persistence_failure_does_not_fail_valid_response() {
    let fixture = Fixture::new();
    let (url, _, task) = upstream(
        JPEG,
        axum::http::StatusCode::OK,
        Some(fixture.root.path().join("images/album.jpg")),
    )
    .await;
    fixture.image_url(&url);
    assert_eq!(
        fixture.manager.read_image("album").await.unwrap().bytes,
        JPEG
    );
    task.abort();
}

#[tokio::test]
async fn missing_image_and_local_io_failure_remain_distinct() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.manager.read_image("album").await,
        Err(MediaReadError::NotFound)
    ));
    std::fs::create_dir_all(fixture.root.path().join("images/album.jpg")).unwrap();
    assert!(matches!(
        fixture.manager.read_image("album").await,
        Err(MediaReadError::Storage(_))
    ));
}

#[tokio::test]
async fn local_audio_ranges_are_bounded_and_missing_track_is_distinct() {
    let fixture = Fixture::new();
    assert!(fixture
        .manager
        .lookup_audio("unknown")
        .await
        .unwrap()
        .is_none());
    let (_, audio) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    let audio = audio.unwrap();
    let metadata = audio.metadata().await.unwrap();
    assert_eq!(metadata.content_length, 10);
    assert_eq!(metadata.content_type, "audio/mpeg");
    let mut stream = audio.range_stream(2, 3).await.unwrap();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(bytes, b"234");
    assert!(!fixture.manager.proxy_enabled());
}

#[cfg(unix)]
#[test]
fn blocking_audio_reader_keeps_validated_file_after_path_replacement() {
    use std::io::Read;
    let fixture = Fixture::new();
    let audio = fixture
        .manager
        .open_local_audio_blocking("track1")
        .unwrap()
        .unwrap();
    let path = fixture.root.path().join("audio/track.mp3");
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("/dev/null", &path).unwrap();
    let (mut reader, filename) = audio.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(filename, "track.mp3");
    assert_eq!(bytes, b"0123456789");
    assert!(fixture.manager.open_local_audio_blocking("track1").is_err());
}

#[tokio::test]
async fn progressive_readers_share_download_and_publication_and_release_on_drop() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.root.path().join("audio/track.mp3")).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Notify::new());
    let count = calls.clone();
    let gate = release.clone();
    let app = axum::Router::new().route(
        "/track/{id}/audio",
        axum::routing::get(move || {
            let count = count.clone();
            let gate = gate.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                let stream =
                    futures::stream::once(async { Ok::<_, io::Error>(Bytes::from_static(b"abc")) })
                        .chain(futures::stream::once(async move {
                            gate.notified().await;
                            Ok::<_, io::Error>(Bytes::from_static(b"def"))
                        }));
                axum::http::Response::builder()
                    .header("content-length", "6")
                    .header("content-type", "audio/mpeg")
                    .header("X-Pezzottify-Audio-Extension", "mp3")
                    .body(axum::body::Body::from_stream(stream))
                    .unwrap()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let registry = DbRegistry::new();
    let search = Arc::new(
        crate::search::Fts5LevenshteinSearchVault::new_lazy(
            &fixture.root.path().join("search.db"),
            &registry,
        )
        .unwrap(),
    );
    let server = Arc::new(
        crate::server_store::SqliteServerStore::new(
            fixture.root.path().join("server.db"),
            &registry,
        )
        .unwrap(),
    );
    fixture.manager.enable_proxy(
        Arc::new(crate::downloader::DownloaderClient::new(url, 10)),
        search,
        server,
        fixture.root.path().to_owned(),
        crate::config::ProxyModeSettings::default(),
    );

    let (_, local) = fixture
        .manager
        .lookup_audio("track1")
        .await
        .unwrap()
        .unwrap();
    assert!(local.is_none());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "local lookup must not fetch"
    );
    let first = fixture
        .manager
        .open_remote_audio("track1", DownloadPriority::Foreground)
        .unwrap();
    let second = fixture
        .manager
        .open_remote_audio("track1", DownloadPriority::Foreground)
        .unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), first.metadata())
            .await
            .unwrap()
            .unwrap()
            .content_length,
        6
    );
    assert_eq!(second.metadata().await.unwrap().content_type, "audio/mpeg");
    let mut first = first.range_stream(0, 4);
    let mut second = second.range_stream(1, 4);
    assert_eq!(first.next().await.unwrap().unwrap(), b"abc".as_slice());
    assert_eq!(second.next().await.unwrap().unwrap(), b"bc".as_slice());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        fixture.manager.proxy_status().unwrap().active[0].active_streams,
        2
    );
    drop(first);
    assert_eq!(
        fixture.manager.proxy_status().unwrap().active[0].active_streams,
        1
    );
    release.notify_one();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), second.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        b"de".as_slice()
    );
    assert!(second.next().await.is_none());
    drop(second);

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if fixture
                .manager
                .lookup_audio("track1")
                .await
                .unwrap()
                .unwrap()
                .1
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("completed remote audio should be published locally");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    task.abort();
}
