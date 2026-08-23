#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::NullCatalogStore;
    use crate::search::{HashedItemType, SearchResult, SearchVault};
    use crate::server_store::{
        JobAuditEntry, JobAuditEventType, JobRun, JobRunStatus, JobScheduleState, ServerStore,
    };
    use crate::user::auth::UserAuthCredentials;
    use crate::user::auth::{AuthToken, AuthTokenValue};
    use crate::user::user_models::{BandwidthSummary, BandwidthUsage, LikedContentType};
    use crate::user::{
        UserAuthCredentialsStore, UserAuthTokenStore, UserBandwidthStore, UserStore,
    };
    use axum::extract::ConnectInfo;
    use axum::{body::Body, http::Request};
    use std::collections::HashMap;
    use std::sync::RwLock;
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready

    #[tokio::test(flavor = "current_thread")]
    async fn password_work_runs_off_the_async_runtime_thread() {
        let runtime_thread = std::thread::current().id();
        let pool = PasswordVerificationPool::with_limits(
            1,
            Duration::from_millis(100),
            Duration::from_secs(1),
        );

        let worker_thread = pool
            .run(|| std::thread::current().id())
            .await
            .expect("password work should complete");

        assert_ne!(worker_thread, runtime_thread);
    }

    #[tokio::test]
    async fn password_work_rejects_when_its_bounded_queue_times_out() {
        let pool = PasswordVerificationPool::with_limits(
            1,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        let first_pool = pool.clone();
        let first_gate = Arc::clone(&gate);
        let first = tokio::spawn(async move {
            first_pool
                .run(move || {
                    started_tx.send(()).expect("test receiver should remain open");
                    let (lock, condvar) = &*first_gate;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = condvar.wait(released).unwrap();
                    }
                })
                .await
        });
        started_rx.await.expect("first job should start");

        let error = pool
            .run(|| ())
            .await
            .expect_err("second job must not bypass the concurrency limit");
        assert_eq!(error, PasswordVerificationError::QueueTimeout);

        let (lock, condvar) = &*gate;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        first.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn password_work_reports_panics_without_panicking_the_runtime() {
        let pool = PasswordVerificationPool::with_limits(
            1,
            Duration::from_millis(100),
            Duration::from_secs(1),
        );

        let error = pool
            .run(|| panic!("sentinel password worker panic"))
            .await
            .expect_err("worker panic should be contained");

        assert_eq!(error, PasswordVerificationError::WorkerPanicked);
    }

    fn valid_listening_request() -> ListeningEventRequest {
        ListeningEventRequest {
            track_id: "track-1".to_string(),
            session_id: Some("session-123".to_string()),
            started_at: Some(780),
            ended_at: Some(1_000),
            duration_seconds: 220,
            track_duration_seconds: 200,
            seek_count: Some(1),
            pause_count: Some(1),
            playback_context: Some("album".to_string()),
            client_type: Some("android".to_string()),
        }
    }

    #[test]
    fn listening_validation_uses_authoritative_duration() {
        assert_eq!(authoritative_track_duration_seconds(240_001), Some(241));
        assert_eq!(authoritative_track_duration_seconds(0), None);

        let validated = validate_listening_event(&valid_listening_request(), 240, 1_000).unwrap();
        assert_eq!(validated.duration_seconds, 220);
        assert!(validated.completed);
    }

    #[test]
    fn listening_validation_rejects_zero_missing_id_and_implausible_values() {
        let mut request = valid_listening_request();
        request.track_duration_seconds = 0;
        assert!(validate_listening_event(&request, 240, 1_000).is_err());

        let mut request = valid_listening_request();
        request.session_id = None;
        assert!(validate_listening_event(&request, 240, 1_000).is_err());

        let mut request = valid_listening_request();
        request.duration_seconds = 251;
        assert!(validate_listening_event(&request, 240, 1_000).is_err());

        let mut request = valid_listening_request();
        request.started_at = Some(1);
        assert!(validate_listening_event(&request, 240, MAX_EVENT_AGE_SECONDS + 2).is_err());
    }

    #[test]
    fn listening_json_rejects_negative_fractional_and_non_finite_numbers() {
        for invalid_duration in ["-1", "1.5", "NaN", "1e999"] {
            let json = format!(
                r#"{{"track_id":"track-1","duration_seconds":{invalid_duration},"track_duration_seconds":240}}"#
            );
            assert!(serde_json::from_str::<ListeningEventRequest>(&json).is_err());
        }
    }

    /// Mock search vault for testing - returns empty results
    struct MockSearchVault;

    impl SearchVault for MockSearchVault {
        fn search(
            &self,
            _query: &str,
            _max_results: usize,
            _filter: Option<Vec<HashedItemType>>,
        ) -> Vec<SearchResult> {
            Vec::new()
        }

        fn rebuild_index(&self) -> anyhow::Result<()> {
            Ok(())
        }

        fn update_popularity(&self, _items: &[(String, HashedItemType, u64, f64)]) {}

        fn upsert_items(&self, _items: &[crate::search::SearchIndexItem]) -> anyhow::Result<()> {
            Ok(())
        }

        fn remove_items(&self, _items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
            Ok(())
        }

        fn get_stats(&self) -> crate::search::SearchVaultStats {
            crate::search::SearchVaultStats {
                indexed_items: 0,
                index_type: "Mock".to_string(),
                state: crate::search::IndexState::Ready,
            }
        }

        fn record_impression(
            &self,
            _item_id: &str,
            _item_type: HashedItemType,
            _source: crate::search::ImpressionSource,
        ) -> bool {
            false
        }

        fn get_impression_totals(
            &self,
            _min_date: i64,
        ) -> std::collections::HashMap<(String, HashedItemType), u64> {
            std::collections::HashMap::new()
        }

        fn prune_impressions(&self, _before_date: i64) -> usize {
            0
        }

        fn update_availability(&self, _items: &[(String, HashedItemType, bool)]) {}
    }

    /// A minimal in-memory ServerStore for testing
    #[derive(Default)]
    struct MockServerStore {
        state: RwLock<HashMap<String, String>>,
    }

    impl ServerStore for MockServerStore {
        fn record_job_start(&self, _job_id: &str, _triggered_by: &str) -> anyhow::Result<i64> {
            Ok(1)
        }
        fn record_job_finish(
            &self,
            _run_id: i64,
            _status: JobRunStatus,
            _error_message: Option<String>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_running_jobs(&self) -> anyhow::Result<Vec<JobRun>> {
            Ok(vec![])
        }
        fn get_job_history(&self, _job_id: &str, _limit: usize) -> anyhow::Result<Vec<JobRun>> {
            Ok(vec![])
        }
        fn get_last_run(&self, _job_id: &str) -> anyhow::Result<Option<JobRun>> {
            Ok(None)
        }
        fn mark_stale_jobs_failed(&self) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn get_schedule_state(&self, _job_id: &str) -> anyhow::Result<Option<JobScheduleState>> {
            Ok(None)
        }
        fn update_schedule_state(&self, _state: &JobScheduleState) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_all_schedule_states(&self) -> anyhow::Result<Vec<JobScheduleState>> {
            Ok(vec![])
        }
        fn get_state(&self, key: &str) -> anyhow::Result<Option<String>> {
            Ok(self.state.read().unwrap().get(key).cloned())
        }
        fn set_state(&self, key: &str, value: &str) -> anyhow::Result<()> {
            self.state
                .write()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete_state(&self, key: &str) -> anyhow::Result<()> {
            self.state.write().unwrap().remove(key);
            Ok(())
        }
        fn log_job_audit(
            &self,
            _job_id: &str,
            _event_type: JobAuditEventType,
            _duration_ms: Option<i64>,
            _details: Option<&serde_json::Value>,
            _error: Option<&str>,
        ) -> anyhow::Result<i64> {
            Ok(1)
        }
        fn get_job_audit_log(
            &self,
            _limit: usize,
            _offset: usize,
        ) -> anyhow::Result<Vec<JobAuditEntry>> {
            Ok(vec![])
        }
        fn get_job_audit_log_by_job(
            &self,
            _job_id: &str,
            _limit: usize,
            _offset: usize,
        ) -> anyhow::Result<Vec<JobAuditEntry>> {
            Ok(vec![])
        }
        fn cleanup_old_job_audit_entries(&self, _before_timestamp: i64) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn insert_bug_report(
            &self,
            _report: &crate::server_store::BugReport,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_bug_report(
            &self,
            _id: &str,
        ) -> anyhow::Result<Option<crate::server_store::BugReport>> {
            Ok(None)
        }
        fn list_bug_reports(
            &self,
            _limit: usize,
            _offset: usize,
        ) -> anyhow::Result<Vec<crate::server_store::BugReportSummary>> {
            Ok(vec![])
        }
        fn delete_bug_report(&self, _id: &str) -> anyhow::Result<bool> {
            Ok(false)
        }
        fn get_bug_reports_total_size(&self) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn cleanup_bug_reports_to_size(&self, _max_size: usize) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn append_catalog_event(
            &self,
            _event_type: crate::server_store::CatalogEventType,
            _content_type: crate::server_store::CatalogContentType,
            _content_id: &str,
            _triggered_by: Option<&str>,
        ) -> anyhow::Result<i64> {
            Ok(1)
        }
        fn get_catalog_events_page(
            &self,
            _since_seq: i64,
            _limit: usize,
        ) -> anyhow::Result<crate::server_store::CatalogEventPage> {
            Ok(crate::server_store::CatalogEventPage {
                events: vec![],
                current_seq: 0,
                has_more: false,
                next_since: 0,
            })
        }
        fn cleanup_old_catalog_events(&self, _before_timestamp: i64) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn add_pending_whatsnew_album(&self, _album_id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_pending_whatsnew_albums(&self) -> anyhow::Result<Vec<(String, i64)>> {
            Ok(vec![])
        }
        fn clear_pending_whatsnew_albums(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn is_album_in_whatsnew(&self, _album_id: &str) -> anyhow::Result<bool> {
            Ok(false)
        }
        fn create_whatsnew_batch(
            &self,
            _id: &str,
            _closed_at: i64,
            _album_ids: &[String],
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn list_whatsnew_batches(
            &self,
            _limit: usize,
        ) -> anyhow::Result<Vec<crate::server_store::WhatsNewBatch>> {
            Ok(vec![])
        }
        fn get_whatsnew_batch_album_ids(&self, _batch_id: &str) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let user_store: Arc<dyn FullUserStore> = Arc::new(InMemoryUserStore::default());
        let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
        let user_manager = Arc::new(crate::user::UserManager::new(user_store.clone()));
        let guarded_search_vault: crate::server::state::GuardedSearchVault =
            Arc::new(MockSearchVault);
        let server_store: Arc<dyn ServerStore> = Arc::new(MockServerStore::default());
        let app = &mut make_app(
            ServerConfig::default(),
            catalog_store,
            guarded_search_vault,
            user_store,
            user_manager,
            None, // no scheduler_handle
            server_store,
            None, // no oidc_config
            Arc::new(crate::backup::DbRegistry::new()),
            None,
        )
        .await
        .unwrap();

        // Create a test socket address for rate limiting
        let test_addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();

        let protected_routes = vec![
            "/v1/content/artist/123",
            "/v1/content/album/123",
            "/v1/content/album/123/resolved",
            "/v1/content/artist/123/discography",
            "/v1/content/track/123",
            "/v1/content/track/123/resolved",
            "/v1/content/image/123",
            "/v1/content/stream/123",
            // Admin routes (require ManagePermissions)
            "/v1/admin/users",
            "/v1/admin/users/testuser/roles",
        ];

        for route in protected_routes.into_iter() {
            println!("Trying route {}", route);
            let mut request = Request::builder().uri(route).body(Body::empty()).unwrap();
            // Add ConnectInfo extension for rate limiting
            request.extensions_mut().insert(ConnectInfo(test_addr));
            let response = app.oneshot(request).await.unwrap();
            // 401 Unauthorized - not authenticated
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/auth/logout")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(test_addr));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test search route
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/content/search")
            .body(Body::empty())
            .unwrap();
        // Add ConnectInfo extension for rate limiting
        request.extensions_mut().insert(ConnectInfo(test_addr));
        let response = app.oneshot(request).await.unwrap();
        // 401 Unauthorized - not authenticated
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[derive(Default)]
    struct InMemoryUserStore {}

    impl UserStore for InMemoryUserStore {
        fn create_user(&self, _user_handle: &str) -> Result<usize> {
            todo!()
        }

        fn delete_user(&self, _user_id: usize) -> Result<bool> {
            todo!()
        }

        fn get_user_handle(&self, _user_id: usize) -> Result<Option<String>> {
            todo!()
        }

        fn get_user_id(&self, _user_handle: &str) -> Result<Option<usize>> {
            todo!()
        }

        fn get_user_id_by_oidc_subject(&self, _oidc_subject: &str) -> Result<Option<usize>> {
            Ok(None)
        }

        fn set_user_oidc_subject(&self, _user_id: usize, _oidc_subject: &str) -> Result<()> {
            Ok(())
        }

        fn get_user_oidc_subject(&self, _user_id: usize) -> Result<Option<String>> {
            Ok(None)
        }

        fn clear_user_oidc_subject(&self, _user_id: usize) -> Result<()> {
            Ok(())
        }

        fn get_user_playlists(&self, _user_id: usize) -> Result<Vec<String>> {
            todo!()
        }

        fn is_user_liked_content(
            &self,
            _user_id: usize,
            _content_id: &str,
        ) -> Result<Option<bool>> {
            todo!()
        }

        fn set_user_liked_content(
            &self,
            _user_id: usize,
            _content_id: &str,
            _content_type: LikedContentType,
            _liked: bool,
        ) -> Result<()> {
            todo!()
        }

        fn get_all_user_handles(&self) -> Result<Vec<String>> {
            todo!()
        }

        fn get_user_liked_content(
            &self,
            _user_id: usize,
            _content_type: LikedContentType,
        ) -> Result<Vec<String>> {
            todo!()
        }

        fn create_user_playlist(
            &self,
            _user_id: usize,
            _playlist_name: &str,
            _creator_id: usize,
            _track_ids: Vec<String>,
        ) -> Result<String> {
            todo!()
        }

        fn delete_user_playlist(&self, _playlist_id: &str, _user_id: usize) -> Result<()> {
            todo!()
        }

        fn update_user_playlist(
            &self,
            _playlist_id: &str,
            _user_id: usize,
            _playlist_name: Option<String>,
            _track_ids: Option<Vec<String>>,
        ) -> Result<()> {
            todo!()
        }

        fn get_user_playlist(
            &self,
            _playlist_id: &str,
            _user_id: usize,
        ) -> Result<crate::user::UserPlaylist> {
            todo!()
        }

        fn get_user_roles(&self, _user_id: usize) -> Result<Vec<crate::user::UserRole>> {
            todo!()
        }

        fn add_user_role(&self, _user_id: usize, _role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn remove_user_role(&self, _user_id: usize, _role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn add_user_extra_permission(
            &self,
            _user_id: usize,
            _grant: crate::user::PermissionGrant,
        ) -> Result<usize> {
            todo!()
        }

        fn remove_user_extra_permission(
            &self,
            _permission_id: usize,
        ) -> Result<Option<(usize, Permission)>> {
            todo!()
        }

        fn decrement_permission_countdown(&self, _permission_id: usize) -> Result<bool> {
            todo!()
        }

        fn resolve_user_permissions(
            &self,
            _user_id: usize,
        ) -> Result<Vec<crate::user::Permission>> {
            Ok(vec![])
        }
    }

    impl UserAuthTokenStore for InMemoryUserStore {
        fn get_user_auth_token(&self, _token: &AuthTokenValue) -> Result<Option<AuthToken>> {
            todo!()
        }

        fn delete_user_auth_token(&self, _token: &AuthTokenValue) -> Result<Option<AuthToken>> {
            todo!()
        }

        fn update_user_auth_token_last_used_timestamp(
            &self,
            _token: &AuthTokenValue,
        ) -> Result<()> {
            todo!()
        }

        fn add_user_auth_token(&self, _token: AuthToken) -> Result<()> {
            todo!()
        }

        fn get_all_user_auth_tokens(&self, _user_handle: &str) -> Result<Vec<AuthToken>> {
            todo!()
        }

        fn prune_unused_auth_tokens(&self, _unused_for_days: u64) -> Result<usize> {
            todo!()
        }
    }

    impl UserAuthCredentialsStore for InMemoryUserStore {
        fn get_user_auth_credentials(
            &self,
            _user_handle: &str,
        ) -> Result<Option<UserAuthCredentials>> {
            todo!()
        }

        fn update_user_auth_credentials(&self, _credentials: UserAuthCredentials) -> Result<()> {
            todo!()
        }
    }

    impl UserBandwidthStore for InMemoryUserStore {
        fn record_bandwidth_usage(
            &self,
            _user_id: usize,
            _date: u32,
            _endpoint_category: &str,
            _bytes_sent: u64,
            _request_count: u64,
        ) -> Result<()> {
            Ok(()) // No-op for tests
        }

        fn get_user_bandwidth_usage(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<BandwidthUsage>> {
            Ok(vec![])
        }

        fn get_user_bandwidth_summary(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<BandwidthSummary> {
            Ok(BandwidthSummary {
                user_id: Some(_user_id),
                total_bytes_sent: 0,
                total_requests: 0,
                by_category: std::collections::HashMap::new(),
            })
        }

        fn get_all_bandwidth_usage(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<BandwidthUsage>> {
            Ok(vec![])
        }

        fn get_total_bandwidth_summary(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<BandwidthSummary> {
            Ok(BandwidthSummary {
                user_id: None,
                total_bytes_sent: 0,
                total_requests: 0,
                by_category: std::collections::HashMap::new(),
            })
        }

        fn prune_bandwidth_usage(&self, _older_than_days: u32) -> Result<usize> {
            Ok(0)
        }
    }

    impl crate::user::UserListeningStore for InMemoryUserStore {
        fn record_listening_event(
            &self,
            _event: crate::user::ListeningEvent,
        ) -> Result<(usize, bool)> {
            Ok((1, true))
        }

        fn get_user_listening_events(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
            _limit: Option<usize>,
            _offset: Option<usize>,
        ) -> Result<Vec<crate::user::ListeningEvent>> {
            Ok(vec![])
        }

        fn get_user_listening_summary(
            &self,
            user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<crate::user::ListeningSummary> {
            Ok(crate::user::ListeningSummary {
                user_id: Some(user_id),
                total_plays: 0,
                total_duration_seconds: 0,
                completed_plays: 0,
                unique_tracks: 0,
            })
        }

        fn get_user_listening_history(
            &self,
            _user_id: usize,
            _limit: usize,
        ) -> Result<Vec<crate::user::UserListeningHistoryEntry>> {
            Ok(vec![])
        }

        fn get_track_listening_stats(
            &self,
            track_id: &str,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<crate::user::TrackListeningStats> {
            Ok(crate::user::TrackListeningStats {
                track_id: track_id.to_string(),
                play_count: 0,
                total_duration_seconds: 0,
                completed_count: 0,
                unique_listeners: 0,
            })
        }

        fn get_daily_listening_stats(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<crate::user::DailyListeningStats>> {
            Ok(vec![])
        }

        fn get_top_tracks(
            &self,
            _start_date: u32,
            _end_date: u32,
            _limit: usize,
        ) -> Result<Vec<crate::user::TrackListeningStats>> {
            Ok(vec![])
        }

        fn get_all_track_play_counts(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<crate::user::user_models::TrackPlayCount>> {
            Ok(vec![])
        }

        fn prune_listening_events(&self, _older_than_days: u32) -> Result<usize> {
            Ok(0)
        }
    }

    impl crate::user::UserSettingsStore for InMemoryUserStore {
        fn get_user_setting(
            &self,
            _user_id: usize,
            _key: &str,
        ) -> Result<Option<crate::user::UserSetting>> {
            Ok(None)
        }

        fn set_user_setting(
            &self,
            _user_id: usize,
            _setting: crate::user::UserSetting,
        ) -> Result<()> {
            Ok(())
        }

        fn get_all_user_settings(&self, _user_id: usize) -> Result<Vec<crate::user::UserSetting>> {
            Ok(vec![])
        }

        fn get_user_ids_with_setting(&self, _key: &str, _value: &str) -> Result<Vec<usize>> {
            Ok(vec![])
        }
    }

    impl crate::user::DeviceStore for InMemoryUserStore {
        fn register_or_update_device(
            &self,
            _registration: &crate::user::device::DeviceRegistration,
        ) -> Result<usize> {
            Ok(1)
        }
        fn get_device(&self, _device_id: usize) -> Result<Option<crate::user::device::Device>> {
            Ok(None)
        }
        fn get_device_by_uuid(
            &self,
            _device_uuid: &str,
        ) -> Result<Option<crate::user::device::Device>> {
            Ok(None)
        }
        fn get_user_devices(&self, _user_id: usize) -> Result<Vec<crate::user::device::Device>> {
            Ok(vec![])
        }
        fn associate_device_with_user(&self, _device_id: usize, _user_id: usize) -> Result<()> {
            Ok(())
        }
        fn touch_device(&self, _device_id: usize) -> Result<()> {
            Ok(())
        }
        fn prune_orphaned_devices(&self, _inactive_for_days: u32) -> Result<usize> {
            Ok(0)
        }
        fn prune_inactive_devices(&self, _inactive_for_days: u32) -> Result<usize> {
            Ok(0)
        }
        fn enforce_user_device_limit(&self, _user_id: usize, _max_devices: usize) -> Result<usize> {
            Ok(0)
        }
        fn get_device_share_policy(
            &self,
            _device_id: usize,
        ) -> Result<crate::user::device::DeviceSharePolicy> {
            Ok(crate::user::device::DeviceSharePolicy::default())
        }
        fn set_device_share_policy(
            &self,
            _device_id: usize,
            _policy: &crate::user::device::DeviceSharePolicy,
        ) -> Result<()> {
            Ok(())
        }
    }

    impl crate::user::UserEventStore for InMemoryUserStore {
        fn append_event(
            &self,
            _user_id: usize,
            event: &crate::user::sync_events::UserEvent,
        ) -> Result<crate::user::sync_events::StoredEvent> {
            Ok(crate::user::sync_events::StoredEvent {
                seq: 1,
                operation_id: None,
                operation_index: 0,
                event: event.clone(),
                server_timestamp: 0,
            })
        }

        fn get_events_since(
            &self,
            _user_id: usize,
            _since_seq: i64,
        ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
            Ok(vec![])
        }

        fn get_current_seq(&self, _user_id: usize) -> Result<i64> {
            Ok(0)
        }

        fn get_min_seq(&self, _user_id: usize) -> Result<Option<i64>> {
            Ok(None)
        }

        fn prune_events_older_than(&self, _before_timestamp: i64) -> Result<u64> {
            Ok(0)
        }
    }

    impl crate::notifications::NotificationStore for InMemoryUserStore {
        fn create_notification(
            &self,
            _user_id: usize,
            notification_type: crate::notifications::NotificationType,
            title: String,
            body: Option<String>,
            data: serde_json::Value,
        ) -> Result<crate::notifications::Notification> {
            Ok(crate::notifications::Notification {
                id: "test-notif-1".to_string(),
                notification_type,
                title,
                body,
                data,
                read_at: None,
                created_at: 0,
            })
        }

        fn get_user_notifications(
            &self,
            _user_id: usize,
        ) -> Result<Vec<crate::notifications::Notification>> {
            Ok(vec![])
        }

        fn get_notification(
            &self,
            _notification_id: &str,
            _user_id: usize,
        ) -> Result<Option<crate::notifications::Notification>> {
            Ok(None)
        }

        fn mark_notification_read(
            &self,
            _notification_id: &str,
            _user_id: usize,
        ) -> Result<Option<crate::notifications::Notification>> {
            Ok(None)
        }

        fn get_unread_count(&self, _user_id: usize) -> Result<usize> {
            Ok(0)
        }
    }

    // Tests for admin endpoints using SqliteUserStore
    mod admin_endpoint_tests {
        use super::*;
        use crate::user::SqliteUserStore;
        use std::time::SystemTime;
        use tempfile::TempDir;

        fn create_test_store() -> (SqliteUserStore, TempDir) {
            let temp_dir = TempDir::new().unwrap();
            let temp_file_path = temp_dir.path().join("test.db");
            let store =
                SqliteUserStore::new(&temp_file_path, &crate::backup::DbRegistry::new()).unwrap();
            (store, temp_dir)
        }

        fn create_test_store_with_admin_user() -> (SqliteUserStore, usize, TempDir) {
            let (store, temp_dir) = create_test_store();
            let user_id = store.create_user("admin_user").unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            (store, user_id, temp_dir)
        }

        #[allow(dead_code)]
        fn create_test_store_with_regular_user() -> (SqliteUserStore, usize, TempDir) {
            let (store, temp_dir) = create_test_store();
            let user_id = store.create_user("regular_user").unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Regular)
                .unwrap();
            (store, user_id, temp_dir)
        }

        #[test]
        fn test_get_all_user_handles() {
            let (store, _temp_dir) = create_test_store();
            store.create_user("user1").unwrap();
            store.create_user("user2").unwrap();
            store.create_user("user3").unwrap();

            let handles = store.get_all_user_handles().unwrap();
            assert_eq!(handles.len(), 3);
            assert!(handles.contains(&"user1".to_string()));
            assert!(handles.contains(&"user2".to_string()));
            assert!(handles.contains(&"user3".to_string()));
        }

        #[test]
        fn test_get_user_id() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let found_id = store.get_user_id("testuser").unwrap();
            assert_eq!(found_id, Some(user_id));

            let not_found = store.get_user_id("nonexistent").unwrap();
            assert_eq!(not_found, None);
        }

        #[test]
        fn test_get_user_roles() {
            let (store, user_id, _temp_dir) = create_test_store_with_admin_user();

            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 1);
            assert_eq!(roles[0], crate::user::UserRole::Admin);
        }

        #[test]
        fn test_add_and_remove_user_role() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            // Add Admin role
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert!(roles.contains(&crate::user::UserRole::Admin));

            // Add Regular role
            store
                .add_user_role(user_id, crate::user::UserRole::Regular)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 2);
            assert!(roles.contains(&crate::user::UserRole::Admin));
            assert!(roles.contains(&crate::user::UserRole::Regular));

            // Remove Admin role
            store
                .remove_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 1);
            assert!(roles.contains(&crate::user::UserRole::Regular));
        }

        #[test]
        fn test_add_duplicate_role_is_idempotent() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();

            let roles = store.get_user_roles(user_id).unwrap();
            // Should still only have one Admin role
            assert_eq!(
                roles
                    .iter()
                    .filter(|r| **r == crate::user::UserRole::Admin)
                    .count(),
                1
            );
        }

        #[test]
        fn test_resolve_user_permissions_from_role() {
            let (store, user_id, _temp_dir) = create_test_store_with_admin_user();

            let permissions = store.resolve_user_permissions(user_id).unwrap();
            // Admin should have: AccessCatalog, EditCatalog, ManagePermissions, ServerAdmin, ViewAnalytics, RequestContent, DownloadManagerAdmin
            assert!(permissions.contains(&crate::user::Permission::AccessCatalog));
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
            assert!(permissions.contains(&crate::user::Permission::ManagePermissions));
            assert!(permissions.contains(&crate::user::Permission::ServerAdmin));
        }

        #[test]
        fn test_add_extra_permission_with_countdown() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: Some(5),
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
            assert!(permission_id > 0);

            // Verify permission is resolved
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_add_extra_permission_with_time_limit() {
            use std::time::Duration;

            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let start_time = SystemTime::now();
            let end_time = start_time + Duration::from_secs(3600); // 1 hour from now

            let grant = crate::user::PermissionGrant::Extra {
                start_time,
                end_time: Some(end_time),
                permission: crate::user::Permission::ServerAdmin,
                countdown: None,
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
            assert!(permission_id > 0);

            // Verify permission is resolved (still within time limit)
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::ServerAdmin));
        }

        #[test]
        fn test_remove_extra_permission() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: None,
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();

            // Verify permission exists
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));

            // Remove it
            store.remove_user_extra_permission(permission_id).unwrap();

            // Verify permission is gone
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(!permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_countdown_decrements_and_removes_permission() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: Some(2),
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();

            // First decrement - should still have uses remaining
            let has_remaining = store.decrement_permission_countdown(permission_id).unwrap();
            assert!(has_remaining);

            // Second decrement - should be last use, permission removed
            let has_remaining = store.decrement_permission_countdown(permission_id).unwrap();
            assert!(!has_remaining);

            // Verify permission is gone
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(!permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_user_manager_get_user_id() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let user_manager = crate::user::UserManager::new(Arc::new(store));

            let found_id = user_manager.get_user_id("testuser").unwrap();
            assert_eq!(found_id, Some(user_id));

            let not_found = user_manager.get_user_id("nonexistent").unwrap();
            assert_eq!(not_found, None);
        }
    }
}
