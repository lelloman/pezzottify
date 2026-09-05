use super::mutations::{copy_path, pending_path, read_record, save, Phase};
use super::tests::Fixture;
use super::*;
use std::io::Read;

fn ready_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .manager
        .configure_search(Arc::new(crate::search::NoopSearchVault));
    fixture
}

#[test]
fn publication_accepts_unicode_identity_and_rejects_forged_receipt_paths() {
    let fixture = ready_fixture();
    let staged = fixture
        .manager
        .begin_publication("copertina è bella", "jpg", Provenance::ImageCache)
        .unwrap();
    drop(staged);
    assert!(fixture
        .manager
        .begin_publication("../outside", "jpg", Provenance::ImageCache)
        .is_err());
    let mut receipt = fixture
        .manager
        .commit_publication(stage(&fixture, b"copy", Provenance::Ingested))
        .unwrap();
    receipt.revision = "../../outside".to_owned();
    assert!(fixture.manager.remove_copy(&receipt).is_err());
    assert!(fixture.manager.retain_copy(&receipt).is_err());
}

#[test]
fn recovery_cleans_abandoned_staging_and_rejects_forged_journal_paths() {
    let fixture = ready_fixture();
    let mut staged = stage(&fixture, b"unfinished", Provenance::Ingested);
    staged.record.owner = "previous-process".to_owned();
    let pending = pending_path(fixture.root.path(), &staged.record.revision);
    save(&pending, &staged.record).unwrap();
    staged.handed_off = true;
    fixture.manager.recover(&|| false).unwrap();
    assert!(!staged.path().exists());
    assert!(!pending.exists());

    staged.record.staging = "../outside".to_owned();
    save(&pending, &staged.record).unwrap();
    assert!(fixture.manager.recover(&|| false).is_err());
    assert!(pending.exists());
}
fn stage(fixture: &Fixture, bytes: &[u8], provenance: Provenance) -> StagedMedia {
    let staged = fixture
        .manager
        .begin_publication("track1", "mp3", provenance)
        .unwrap();
    std::fs::write(staged.path(), bytes).unwrap();
    staged
}
#[test]
fn replacement_preserves_old_reader_and_failed_staging_preserves_current_copy() {
    let fixture = ready_fixture();
    let (mut old, _) = fixture
        .manager
        .open_local_audio_blocking("track1")
        .unwrap()
        .unwrap()
        .into_reader();
    let receipt = fixture
        .manager
        .commit_publication(stage(&fixture, b"new complete bytes", Provenance::Ingested))
        .unwrap();
    let mut old_bytes = Vec::new();
    old.read_to_end(&mut old_bytes).unwrap();
    assert_eq!(old_bytes, b"0123456789");
    let incomplete = fixture
        .manager
        .begin_publication("track1", "mp3", Provenance::Ingested)
        .unwrap();
    assert!(fixture.manager.commit_publication(incomplete).is_err());
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri
            .as_deref(),
        Some(receipt.uri.as_str())
    );
}
#[test]
fn stale_publication_and_retention_cannot_destroy_ingested_replacement() {
    let fixture = ready_fixture();
    let proxy = fixture
        .manager
        .commit_publication(stage(
            &fixture,
            b"proxy",
            Provenance::Proxy { materialized_at: 7 },
        ))
        .unwrap();
    let stale = stage(&fixture, b"late", Provenance::Ingested);
    let protected = fixture
        .manager
        .commit_publication(stage(&fixture, b"protected", Provenance::Ingested))
        .unwrap();
    assert!(fixture.manager.commit_publication(stale).is_err());
    assert!(!fixture.manager.remove_copy(&proxy).unwrap());
    assert!(!fixture.manager.remove_copy(&protected).unwrap());
    assert_eq!(
        std::fs::read(fixture.root.path().join(&protected.uri)).unwrap(),
        b"protected"
    );
}
#[test]
fn removal_is_idempotent_and_keeps_active_reader_valid() {
    let fixture = ready_fixture();
    let copy = fixture
        .manager
        .commit_publication(stage(
            &fixture,
            b"proxy",
            Provenance::Proxy { materialized_at: 7 },
        ))
        .unwrap();
    let (mut reader, _) = fixture
        .manager
        .open_local_audio_blocking("track1")
        .unwrap()
        .unwrap()
        .into_reader();
    assert!(fixture.manager.remove_copy(&copy).unwrap());
    assert!(!fixture.manager.remove_copy(&copy).unwrap());
    assert!(fixture
        .manager
        .catalog
        .get_track("track1")
        .unwrap()
        .unwrap()
        .audio_uri
        .is_none());
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"proxy");
}
#[test]
fn crash_after_ready_intent_recovers_publication_and_pending_effects() {
    let fixture = ready_fixture();
    let mut staged = stage(&fixture, b"recover me", Provenance::Ingested);
    staged.record.phase = Phase::Ready;
    save(
        &pending_path(fixture.root.path(), &staged.record.revision),
        &staged.record,
    )
    .unwrap();
    let revision = staged.record.revision.clone();
    // Simulate process termination rather than normal lease cancellation.
    staged.handed_off = true;
    drop(staged);
    let restarted = MediaManager::new(
        fixture.manager.catalog.clone(),
        crate::db_executor::DbExecutor::new(Default::default()),
    );
    // Catalog/file commitment survives an unavailable secondary store.
    assert!(restarted.recover(&|| false).is_err());
    assert!(pending_path(fixture.root.path(), &revision).exists());
    restarted.configure_search(Arc::new(crate::search::NoopSearchVault));
    assert_eq!(restarted.recover(&|| false).unwrap(), 1);
    assert_eq!(restarted.recover(&|| false).unwrap(), 0);
    let record = read_record(&copy_path(fixture.root.path(), &revision)).unwrap();
    assert_eq!(
        std::fs::read(fixture.root.path().join(record.uri)).unwrap(),
        b"recover me"
    );
}
#[test]
fn crash_during_removal_recovers_without_detaching_a_newer_copy() {
    let fixture = ready_fixture();
    let mut removed = fixture
        .manager
        .commit_publication(stage(
            &fixture,
            b"old proxy",
            Provenance::Proxy { materialized_at: 7 },
        ))
        .unwrap();
    removed.phase = Phase::Removing;
    save(
        &pending_path(fixture.root.path(), &removed.revision),
        &removed,
    )
    .unwrap();
    let new = fixture
        .manager
        .commit_publication(stage(&fixture, b"new protected", Provenance::Ingested))
        .unwrap();
    fixture.manager.recover(&|| false).unwrap();
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri
            .as_deref(),
        Some(new.uri.as_str())
    );
    assert!(fixture.root.path().join(new.uri).exists());
    assert!(!fixture.root.path().join(removed.uri).exists());
}
#[test]
fn legacy_copy_is_protected_and_retained_proxy_loses_eviction_eligibility() {
    let fixture = ready_fixture();
    assert!(fixture.manager.proxy_copy("track1", 7).unwrap().is_none());
    let copy = fixture
        .manager
        .commit_publication(stage(
            &fixture,
            b"keep",
            Provenance::Proxy { materialized_at: 7 },
        ))
        .unwrap();
    fixture.manager.retain_copy(&copy).unwrap();
    assert!(fixture.manager.proxy_copy("track1", 7).unwrap().is_none());
    assert!(!fixture.manager.remove_copy(&copy).unwrap());
}
#[test]
fn stale_presence_observation_does_not_overwrite_new_attachment() {
    let fixture = ready_fixture();
    let previous = fixture
        .manager
        .catalog
        .get_track("track1")
        .unwrap()
        .unwrap()
        .audio_uri;
    let copy = fixture
        .manager
        .commit_publication(stage(&fixture, b"new", Provenance::Ingested))
        .unwrap();
    let result = fixture
        .manager
        .catalog
        .apply_media_observations(&[("track1".into(), previous, false)], &|| false)
        .unwrap();
    assert_eq!(result.repaired.tracks_updated, 0);
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri
            .as_deref(),
        Some(copy.uri.as_str())
    );
}
#[cfg(unix)]
#[test]
fn unknown_presence_does_not_repair_persisted_availability_as_missing() {
    let fixture = ready_fixture();
    std::fs::remove_file(fixture.root.path().join("audio/track.mp3")).unwrap();
    std::os::unix::fs::symlink("/dev/null", fixture.root.path().join("audio/track.mp3")).unwrap();
    assert!(matches!(
        probe(fixture.root.path(), Some("audio/track.mp3")),
        MediaPresence::Unknown(_)
    ));
    let result = fixture.manager.reconcile_presence(&|| false).unwrap();
    assert_eq!(result.repaired.tracks_updated, 0);
}
#[test]
fn cancelled_presence_scan_does_not_apply_repairs() {
    let fixture = ready_fixture();
    std::fs::remove_file(fixture.root.path().join("audio/track.mp3")).unwrap();
    assert!(fixture.manager.reconcile_presence(&|| true).is_err());
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .availability,
        crate::catalog_store::TrackAvailability::Available
    );
    let result = fixture.manager.reconcile_presence(&|| false).unwrap();
    assert_eq!(result.repaired.tracks_updated, 1);
}

#[test]
fn failed_catalog_attachment_preserves_previous_copy_and_recovers_after_retry() {
    let fixture = ready_fixture();
    let conn = rusqlite::Connection::open(fixture.root.path().join("catalog.db")).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_attachment BEFORE UPDATE OF audio_uri ON tracks BEGIN SELECT RAISE(ABORT, 'injected attachment failure'); END;").unwrap();
    let prepared = stage(&fixture, b"replacement", Provenance::Ingested);
    let revision = prepared.record.revision.clone();
    assert!(fixture.manager.commit_publication(prepared).is_err());
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri
            .as_deref(),
        Some("audio/track.mp3")
    );
    conn.execute_batch("DROP TRIGGER reject_attachment")
        .unwrap();
    assert_eq!(fixture.manager.recover(&|| false).unwrap(), 1);
    assert!(read_record(&copy_path(fixture.root.path(), &revision)).is_ok());
}

#[test]
fn committed_copy_explicitly_reports_pending_secondary_work() {
    let fixture = Fixture::new();
    let receipt = fixture
        .manager
        .commit_publication(stage(&fixture, b"durable", Provenance::Ingested))
        .unwrap();
    assert!(receipt.pending_effects);
    assert!(pending_path(fixture.root.path(), &receipt.revision).exists());
    fixture
        .manager
        .configure_search(Arc::new(crate::search::NoopSearchVault));
    fixture.manager.recover(&|| false).unwrap();
    assert!(!pending_path(fixture.root.path(), &receipt.revision).exists());
}

#[tokio::test]
async fn ingestion_failure_stays_retryable_and_successful_retry_completes_once() {
    use crate::ingestion::*;
    let fixture = ready_fixture();
    let registry = crate::backup::DbRegistry::new();
    let store = Arc::new(
        SqliteIngestionStore::open(&fixture.root.path().join("ingestion.db"), &registry).unwrap(),
    );
    let mut job = IngestionJob::new("job", "user", "input.mp3", 10, 1);
    job.status = IngestionJobStatus::Converting;
    job.matched_album_id = Some("album".into());
    job.tracks_matched = 1;
    store.create_job(&job).unwrap();
    let input = fixture.root.path().join("input.mp3");
    std::fs::write(&input, b"uploaded audio").unwrap();
    let mut file = IngestionFile::new("file", "job", "input.mp3", 14, input.to_string_lossy());
    file.matched_track_id = Some("track1".into());
    file.conversion_reason = Some(ConversionReason::NoConversionNeeded);
    store.create_file(&file).unwrap();
    let ingestion = IngestionManager::new(
        store.clone(),
        fixture.manager.catalog.clone(),
        Arc::new(crate::search::NoopSearchVault),
        IngestionManagerConfig {
            media_dir: fixture.root.path().to_owned(),
            temp_dir: fixture.root.path().join("uploads"),
            ..Default::default()
        },
        None,
    )
    .with_media(fixture.manager.clone());
    let conn = rusqlite::Connection::open(fixture.root.path().join("catalog.db")).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_attachment BEFORE UPDATE OF audio_uri ON tracks BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(ingestion.convert_job("job").await.is_err());
    assert_eq!(
        store.get_job("job").unwrap().unwrap().status,
        IngestionJobStatus::Converting
    );
    assert!(!store.get_file("file").unwrap().unwrap().converted);
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri
            .as_deref(),
        Some("audio/track.mp3")
    );
    conn.execute_batch("DROP TRIGGER reject_attachment")
        .unwrap();
    ingestion.convert_job("job").await.unwrap();
    let committed = fixture
        .manager
        .catalog
        .get_track("track1")
        .unwrap()
        .unwrap()
        .audio_uri;
    ingestion.convert_job("job").await.unwrap();
    assert_eq!(
        fixture
            .manager
            .catalog
            .get_track("track1")
            .unwrap()
            .unwrap()
            .audio_uri,
        committed
    );
    assert_eq!(
        store.get_job("job").unwrap().unwrap().status,
        IngestionJobStatus::Completed
    );
    fixture.manager.recover(&|| false).unwrap(); // earlier failed revision cannot supersede retry
}
