use std::sync::Arc;

use pezzottify_server::background_jobs::{
    jobs::{DevicePruningJob, WhatsNewBatchJob},
    BackgroundJob, JobContext,
};
use pezzottify_server::backup::DbRegistry;
use pezzottify_server::catalog_store::{CatalogStore, NullCatalogStore};
use pezzottify_server::config::DevicePruningJobSettings;
use pezzottify_server::server_store::{JobAuditEventType, ServerStore, SqliteServerStore};
use pezzottify_server::user::{
    device::{DeviceRegistration, DeviceType},
    DeviceStore, FullUserStore, SqliteUserStore, UserManager,
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

struct JobFixture {
    _temp_dir: TempDir,
    user_db_path: std::path::PathBuf,
    user_store: Arc<SqliteUserStore>,
    server_store: Arc<SqliteServerStore>,
    context: JobContext,
}

fn job_fixture() -> JobFixture {
    let temp_dir = tempfile::tempdir().unwrap();
    let registry = DbRegistry::new();
    let user_db_path = temp_dir.path().join("user.db");
    let user_store = Arc::new(SqliteUserStore::new(&user_db_path, &registry).unwrap());
    let server_store =
        Arc::new(SqliteServerStore::new(temp_dir.path().join("server.db"), &registry).unwrap());
    let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
    let full_user_store: Arc<dyn FullUserStore> = user_store.clone();
    let server: Arc<dyn ServerStore> = server_store.clone();
    let user_manager = Arc::new(UserManager::new(full_user_store.clone()));
    let context = JobContext::new(
        CancellationToken::new(),
        catalog_store,
        full_user_store,
        server,
        user_manager,
    );

    JobFixture {
        _temp_dir: temp_dir,
        user_db_path,
        user_store,
        server_store,
        context,
    }
}

#[test]
fn device_pruning_removes_only_stale_devices_and_records_audit_contract() {
    let fixture = job_fixture();
    let stale_id = fixture
        .user_store
        .register_or_update_device(&DeviceRegistration {
            device_uuid: "stale-device".to_string(),
            device_type: DeviceType::Web,
            device_name: Some("Stale".to_string()),
            os_info: None,
        })
        .unwrap();
    let fresh_id = fixture
        .user_store
        .register_or_update_device(&DeviceRegistration {
            device_uuid: "fresh-device".to_string(),
            device_type: DeviceType::Android,
            device_name: Some("Fresh".to_string()),
            os_info: None,
        })
        .unwrap();

    rusqlite::Connection::open(&fixture.user_db_path)
        .unwrap()
        .execute(
            "UPDATE device SET last_seen = 0 WHERE id = ?1",
            rusqlite::params![stale_id],
        )
        .unwrap();

    DevicePruningJob::from_settings(&DevicePruningJobSettings {
        interval_hours: 24,
        retention_days: 90,
    })
    .execute(&fixture.context)
    .unwrap();

    assert!(fixture.user_store.get_device(stale_id).unwrap().is_none());
    assert!(fixture.user_store.get_device(fresh_id).unwrap().is_some());
    let audit = fixture
        .server_store
        .get_job_audit_log_by_job("device_pruning", 10, 0)
        .unwrap();
    assert_eq!(audit.len(), 2);
    assert!(audit
        .iter()
        .any(|entry| entry.event_type == JobAuditEventType::Started));
    let completed = audit
        .iter()
        .find(|entry| entry.event_type == JobAuditEventType::Completed)
        .unwrap();
    assert_eq!(completed.details.as_ref().unwrap()["devices_deleted"], 1);
}

#[test]
fn whatsnew_batch_moves_pending_albums_once_and_records_audit_contract() {
    let fixture = job_fixture();
    fixture
        .server_store
        .add_pending_whatsnew_album("album-a")
        .unwrap();
    fixture
        .server_store
        .add_pending_whatsnew_album("album-b")
        .unwrap();

    WhatsNewBatchJob::with_interval_hours(6)
        .execute(&fixture.context)
        .unwrap();

    assert!(fixture
        .server_store
        .get_pending_whatsnew_albums()
        .unwrap()
        .is_empty());
    let batches = fixture.server_store.list_whatsnew_batches(10).unwrap();
    assert_eq!(batches.len(), 1);
    let mut album_ids = fixture
        .server_store
        .get_whatsnew_batch_album_ids(&batches[0].id)
        .unwrap();
    album_ids.sort();
    assert_eq!(album_ids, ["album-a", "album-b"]);

    WhatsNewBatchJob::with_interval_hours(6)
        .execute(&fixture.context)
        .unwrap();
    assert_eq!(
        fixture
            .server_store
            .list_whatsnew_batches(10)
            .unwrap()
            .len(),
        1
    );

    let audit = fixture
        .server_store
        .get_job_audit_log_by_job("whatsnew_batch", 10, 0)
        .unwrap();
    assert_eq!(
        audit
            .iter()
            .filter(|entry| entry.event_type == JobAuditEventType::Started)
            .count(),
        2
    );
    assert_eq!(
        audit
            .iter()
            .filter(|entry| entry.event_type == JobAuditEventType::Completed)
            .count(),
        2
    );
    assert!(audit.iter().any(|entry| {
        entry.event_type == JobAuditEventType::Completed
            && entry.details.as_ref().is_some_and(|details| {
                details.get("batch_created") == Some(&serde_json::Value::Bool(false))
            })
    }));
}

#[test]
fn cancelled_lightweight_job_does_not_touch_data_or_write_audit_rows() {
    let fixture = job_fixture();
    fixture
        .server_store
        .add_pending_whatsnew_album("album-pending")
        .unwrap();
    fixture.context.cancellation_token.cancel();

    let result = WhatsNewBatchJob::new().execute(&fixture.context);

    assert!(matches!(
        result,
        Err(pezzottify_server::background_jobs::JobError::Cancelled)
    ));
    assert_eq!(
        fixture
            .server_store
            .get_pending_whatsnew_albums()
            .unwrap()
            .len(),
        1
    );
    assert!(fixture
        .server_store
        .get_job_audit_log_by_job("whatsnew_batch", 10, 0)
        .unwrap()
        .is_empty());
}
