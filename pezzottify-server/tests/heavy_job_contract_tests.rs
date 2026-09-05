use std::sync::Arc;

use pezzottify_server::background_jobs::{
    jobs::{
        CatalogAvailabilityStatsJob, CatalogAvailabilityStatsSnapshot, CatalogCardinalityStatsJob,
    },
    BackgroundJob, JobContext, JobError,
};
use pezzottify_server::backup::DbRegistry;
use pezzottify_server::catalog_store::{
    Album, AlbumAvailability, AlbumType, Artist, CatalogStore, SqliteCatalogStore, Track,
    TrackAvailability,
};
use pezzottify_server::config::CatalogAvailabilityStatsJobSettings;
use pezzottify_server::server_store::{JobAuditEventType, ServerStore, SqliteServerStore};
use pezzottify_server::user::{FullUserStore, SqliteUserStore, UserManager};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

struct JobFixture {
    temp_dir: TempDir,
    catalog_store: Arc<SqliteCatalogStore>,
    server_store: Arc<SqliteServerStore>,
    context: JobContext,
}

fn job_fixture() -> JobFixture {
    let temp_dir = tempfile::tempdir().unwrap();
    let registry = DbRegistry::new();
    let catalog_store = Arc::new(
        SqliteCatalogStore::new(
            temp_dir.path().join("catalog.db"),
            temp_dir.path(),
            2,
            &registry,
        )
        .unwrap(),
    );
    let user_store =
        Arc::new(SqliteUserStore::new(temp_dir.path().join("user.db"), &registry).unwrap());
    let server_store =
        Arc::new(SqliteServerStore::new(temp_dir.path().join("server.db"), &registry).unwrap());
    let catalog: Arc<dyn CatalogStore> = catalog_store.clone();
    let users: Arc<dyn FullUserStore> = user_store;
    let server: Arc<dyn ServerStore> = server_store.clone();
    let user_manager = Arc::new(UserManager::new(users.clone()));
    let context = JobContext::with_search_vault(
        CancellationToken::new(),
        catalog,
        users,
        server,
        user_manager,
        Arc::new(pezzottify_server::search::NoopSearchVault),
    );

    JobFixture {
        temp_dir,
        catalog_store,
        server_store,
        context,
    }
}

fn seed_catalog(fixture: &JobFixture, audio_uri: Option<&str>) {
    fixture
        .catalog_store
        .create_artist(&Artist {
            id: "artist-1".to_owned(),
            name: "Artist One".to_owned(),
            genres: vec!["test".to_owned()],
            followers_total: 0,
            popularity: 0,
            available: true,
        })
        .unwrap();
    fixture
        .catalog_store
        .create_album(
            &Album {
                id: "album-1".to_owned(),
                name: "Album One".to_owned(),
                album_type: AlbumType::Album,
                label: None,
                release_date: Some("2026".to_owned()),
                release_date_precision: Some("year".to_owned()),
                external_id_upc: None,
                popularity: 0,
                album_availability: AlbumAvailability::Complete,
            },
            &["artist-1".to_owned()],
        )
        .unwrap();
    fixture
        .catalog_store
        .create_track(
            &Track {
                id: "track-1".to_owned(),
                name: "Track One".to_owned(),
                album_id: "album-1".to_owned(),
                disc_number: 1,
                track_number: 1,
                duration_ms: 1_000,
                explicit: false,
                popularity: 0,
                language: None,
                external_id_isrc: None,
                audio_uri: audio_uri.map(str::to_owned),
                availability: TrackAvailability::Available,
            },
            &["artist-1".to_owned()],
        )
        .unwrap();
    if let Some(audio_uri) = audio_uri {
        let audio_path = fixture.temp_dir.path().join(audio_uri);
        std::fs::create_dir_all(audio_path.parent().unwrap()).unwrap();
        std::fs::write(&audio_path, b"audio").unwrap();
        fixture
            .catalog_store
            .set_track_audio_uri("track-1", audio_uri)
            .unwrap();
        std::fs::remove_file(audio_path).unwrap();
    }
}

#[test]
fn cardinality_job_rebuilds_counts_and_records_the_result() {
    let fixture = job_fixture();
    seed_catalog(&fixture, None);

    CatalogCardinalityStatsJob
        .execute(&fixture.context)
        .unwrap();

    let stats = fixture
        .catalog_store
        .get_catalog_cardinality_stats()
        .unwrap()
        .unwrap();
    assert_eq!((stats.artists, stats.albums, stats.tracks), (1, 1, 1));

    let audit = fixture
        .server_store
        .get_job_audit_log_by_job("catalog_cardinality_stats", 10, 0)
        .unwrap();
    assert_eq!(audit.len(), 2);
    assert!(audit
        .iter()
        .any(|entry| entry.event_type == JobAuditEventType::Started));
    let completed = audit
        .iter()
        .find(|entry| entry.event_type == JobAuditEventType::Completed)
        .unwrap();
    let details = completed.details.as_ref().unwrap();
    assert_eq!(details["artists"], 1);
    assert_eq!(details["albums"], 1);
    assert_eq!(details["tracks"], 1);
    assert!(details["mutation_version"].as_i64().is_some());
}

#[test]
fn availability_job_reconciles_missing_media_and_persists_a_snapshot() {
    let fixture = job_fixture();
    seed_catalog(&fixture, Some("missing/track-1.ogg"));

    CatalogAvailabilityStatsJob::from_settings(&CatalogAvailabilityStatsJobSettings {
        interval_hours: 24,
        startup_delay_minutes: 0,
    })
    .execute(&fixture.context)
    .unwrap();

    let raw_snapshot = fixture
        .server_store
        .get_state(CatalogAvailabilityStatsJob::snapshot_state_key())
        .unwrap()
        .unwrap();
    let snapshot: CatalogAvailabilityStatsSnapshot = serde_json::from_str(&raw_snapshot).unwrap();
    assert_eq!(snapshot.job.id, "catalog_availability_stats");
    assert_eq!(snapshot.job.version, 1);
    assert_eq!(snapshot.counts.tracks.total, 1);
    assert_eq!(snapshot.counts.tracks.available, 0);
    assert_eq!(snapshot.counts.tracks.unavailable, 1);
    assert_eq!(snapshot.counts.albums.available, 0);
    assert_eq!(snapshot.counts.artists.available, 0);

    let track = fixture.catalog_store.get_track("track-1").unwrap().unwrap();
    assert_eq!(track.availability, TrackAvailability::Unavailable);
    let album = fixture.catalog_store.get_album("album-1").unwrap().unwrap();
    assert_eq!(album.album_availability, AlbumAvailability::Missing);
    let artist = fixture
        .catalog_store
        .get_artist("artist-1")
        .unwrap()
        .unwrap();
    assert!(!artist.available);

    let audit = fixture
        .server_store
        .get_job_audit_log_by_job("catalog_availability_stats", 10, 0)
        .unwrap();
    let completed = audit
        .iter()
        .find(|entry| entry.event_type == JobAuditEventType::Completed)
        .unwrap();
    assert_eq!(
        completed.details.as_ref().unwrap()["repaired"]["tracks_updated"],
        1
    );
}

#[test]
fn cancelled_cardinality_job_fails_audit_without_rebuilding() {
    let fixture = job_fixture();
    seed_catalog(&fixture, None);
    let before = fixture
        .catalog_store
        .get_catalog_cardinality_stats()
        .unwrap()
        .unwrap();
    fixture.context.cancellation_token.cancel();

    let result = CatalogCardinalityStatsJob.execute(&fixture.context);

    assert!(matches!(result, Err(JobError::Cancelled)));
    assert_eq!(
        fixture
            .catalog_store
            .get_catalog_cardinality_stats()
            .unwrap()
            .unwrap(),
        before
    );
    let audit = fixture
        .server_store
        .get_job_audit_log_by_job("catalog_cardinality_stats", 10, 0)
        .unwrap();
    assert_eq!(audit.len(), 2);
    assert!(audit
        .iter()
        .any(|entry| entry.event_type == JobAuditEventType::Started));
    assert!(audit
        .iter()
        .any(|entry| entry.event_type == JobAuditEventType::Failed
            && entry.error.as_deref() == Some("Cancelled")));
    assert!(!audit
        .iter()
        .any(|entry| entry.event_type == JobAuditEventType::Completed));
}
