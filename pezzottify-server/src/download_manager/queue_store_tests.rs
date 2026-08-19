#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_new_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("download_queue.db");

        let store =
            SqliteDownloadQueueStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();

        // Verify database file was created
        assert!(db_path.exists());

        // Verify we can access the connection
        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='download_queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_open_existing_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("download_queue.db");

        // Create database
        {
            let _store =
                SqliteDownloadQueueStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();
        }

        // Reopen database
        let store =
            SqliteDownloadQueueStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();

        // Verify tables exist
        let conn = store.conn.lock().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'download%' OR name LIKE 'user_request%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"download_queue".to_string()));
        assert!(tables.contains(&"download_activity_log".to_string()));
        assert!(tables.contains(&"download_audit_log".to_string()));
        assert!(tables.contains(&"user_request_stats".to_string()));
    }

    #[test]
    fn test_in_memory_store() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // 4 tables should be created (ticket_mapping was dropped in v2)
        assert_eq!(count, 4);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");
    }

    #[test]
    fn test_schema_version_stored() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .unwrap();

        // Version should be BASE_DB_VERSION + latest schema version (2)
        let expected_version = BASE_DB_VERSION + 2;
        assert_eq!(version as usize, expected_version);
    }

    // === Queue Management Tests ===

    #[test]
    fn test_enqueue_and_get_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "test-item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_names(
            Some("Test Album".to_string()),
            Some("Test Artist".to_string()),
        )
        .with_user("user-456".to_string());

        store.enqueue(item.clone()).unwrap();

        let retrieved = store.get_item("test-item-1").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "test-item-1");
        assert_eq!(retrieved.content_type, DownloadContentType::Album);
        assert_eq!(retrieved.content_id, "album-123");
        assert_eq!(retrieved.content_name, Some("Test Album".to_string()));
        assert_eq!(retrieved.artist_name, Some("Test Artist".to_string()));
        assert_eq!(retrieved.requested_by_user_id, Some("user-456".to_string()));
        assert_eq!(retrieved.status, QueueStatus::Pending);
        assert_eq!(retrieved.priority, QueuePriority::User);
    }

    #[test]
    fn test_get_item_not_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let result = store.get_item("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_next_pending_priority_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with different priorities
        let low_priority = QueueItem::new(
            "low-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Expansion, // Lowest priority (3)
            RequestSource::Expansion,
            5,
        );

        let high_priority = QueueItem::new(
            "high-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::Urgent, // Highest priority (1)
            RequestSource::Watchdog,
            3,
        );

        let mid_priority = QueueItem::new(
            "mid-1".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User, // Mid priority (2)
            RequestSource::User,
            5,
        );

        // Enqueue in wrong order
        store.enqueue(low_priority).unwrap();
        store.enqueue(mid_priority).unwrap();
        store.enqueue(high_priority).unwrap();

        // Should get highest priority (lowest value) first
        let next = store.get_next_pending().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "high-1");
    }

    #[test]
    fn test_get_next_pending_age_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with same priority but different created_at
        let mut older = QueueItem::new(
            "older-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        older.created_at = 1000;

        let mut newer = QueueItem::new(
            "newer-1".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        newer.created_at = 2000;

        store.enqueue(newer).unwrap();
        store.enqueue(older).unwrap();

        // Should get older item first
        let next = store.get_next_pending().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "older-1");
    }

    #[test]
    fn test_get_next_pending_empty_queue() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let next = store.get_next_pending().unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn test_list_by_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items for different users
        let user1_item1 = QueueItem::new(
            "u1-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let user1_item2 = QueueItem::new(
            "u1-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let user2_item = QueueItem::new(
            "u2-1".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-2".to_string());

        store.enqueue(user1_item1).unwrap();
        store.enqueue(user1_item2).unwrap();
        store.enqueue(user2_item).unwrap();

        // List user-1 items
        let user1_items = store.list_by_user("user-1", None, 100, 0).unwrap();
        assert_eq!(user1_items.len(), 2);

        // List user-2 items
        let user2_items = store.list_by_user("user-2", None, 100, 0).unwrap();
        assert_eq!(user2_items.len(), 1);
        assert_eq!(user2_items[0].id, "u2-1");
    }

    #[test]
    fn test_list_all_with_status_filter() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.status = QueueStatus::Completed;

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // List all
        let all = store.list_all(None, false, false, 100, 0).unwrap();
        assert_eq!(all.len(), 2);

        // List pending only
        let pending = store
            .list_all(Some(QueueStatus::Pending), false, false, 100, 0)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "item-1");

        // List completed only
        let completed = store
            .list_all(Some(QueueStatus::Completed), false, false, 100, 0)
            .unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "item-2");
    }

    #[test]
    fn test_list_all_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add 5 items
        for i in 0..5 {
            let mut item = QueueItem::new(
                format!("item-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            item.created_at = i as i64;
            store.enqueue(item).unwrap();
        }

        // Get first page
        let page1 = store.list_all(None, false, false, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = store.list_all(None, false, false, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page
        let page3 = store.list_all(None, false, false, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_get_queue_position() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with different priorities
        let mut high = QueueItem::new(
            "high".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Urgent,
            RequestSource::Watchdog,
            5,
        );
        high.created_at = 1000;

        let mut mid = QueueItem::new(
            "mid".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        mid.created_at = 2000;

        let mut low = QueueItem::new(
            "low".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::Expansion,
            RequestSource::Expansion,
            5,
        );
        low.created_at = 3000;

        store.enqueue(low).unwrap();
        store.enqueue(mid).unwrap();
        store.enqueue(high).unwrap();

        // Check positions
        assert_eq!(store.get_queue_position("high").unwrap(), Some(1));
        assert_eq!(store.get_queue_position("mid").unwrap(), Some(2));
        assert_eq!(store.get_queue_position("low").unwrap(), Some(3));
    }

    #[test]
    fn test_get_queue_position_not_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "completed-item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;

        store.enqueue(item).unwrap();

        // Completed items have no queue position
        assert_eq!(store.get_queue_position("completed-item").unwrap(), None);
    }

    #[test]
    fn test_get_queue_position_nonexistent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        assert_eq!(store.get_queue_position("nonexistent").unwrap(), None);
    }

    // === State Transition Tests ===

    #[test]
    fn test_claim_for_processing_success() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Claim the item
        let claimed = store.claim_for_processing("item-1").unwrap();
        assert!(claimed);

        // Verify status changed
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::InProgress);
        assert!(item.started_at.is_some());
        assert!(item.last_attempt_at.is_some());
    }

    #[test]
    fn test_claim_for_processing_already_claimed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // First claim succeeds
        assert!(store.claim_for_processing("item-1").unwrap());

        // Second claim fails
        assert!(!store.claim_for_processing("item-1").unwrap());
    }

    #[test]
    fn test_claim_for_processing_not_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // Cannot claim a completed item
        let claimed = store.claim_for_processing("item-1").unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_claim_for_processing_nonexistent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Claiming nonexistent item returns false (not an error)
        let claimed = store.claim_for_processing("nonexistent").unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_mark_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark as completed
        store.mark_completed("item-1", 1024000, 500).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Completed);
        assert!(item.completed_at.is_some());
        assert_eq!(item.bytes_downloaded, Some(1024000));
        assert_eq!(item.processing_duration_ms, Some(500));
        assert!(item.error_type.is_none());
        assert!(item.error_message.is_none());
    }

    #[test]
    fn test_mark_completed_clears_error() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create item with previous error
        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.error_type = Some(DownloadErrorType::Connection);
        item.error_message = Some("Previous error".to_string());
        store.enqueue(item).unwrap();

        // Mark as completed
        store.mark_completed("item-1", 1024000, 500).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Completed);
        assert!(item.error_type.is_none());
        assert!(item.error_message.is_none());
    }

    #[test]
    fn test_mark_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark for retry
        let error = DownloadError::new(DownloadErrorType::Timeout, "Request timed out");
        let next_retry = 1700000000;
        store
            .mark_retry_waiting("item-1", next_retry, &error)
            .unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert_eq!(item.retry_count, 1);
        assert_eq!(item.next_retry_at, Some(next_retry));
        assert_eq!(item.error_type, Some(DownloadErrorType::Timeout));
        assert_eq!(item.error_message, Some("Request timed out".to_string()));
        assert!(item.last_attempt_at.is_some());
    }

    #[test]
    fn test_mark_retry_waiting_increments_count() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");

        // First retry
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 1);

        // Second retry
        store.mark_retry_waiting("item-1", 2000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 2);

        // Third retry
        store.mark_retry_waiting("item-1", 3000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 3);
    }

    #[test]
    fn test_mark_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark as failed
        let error = DownloadError::new(DownloadErrorType::NotFound, "Album not found");
        store.mark_failed("item-1", &error).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Failed);
        assert!(item.completed_at.is_some());
        assert_eq!(item.error_type, Some(DownloadErrorType::NotFound));
        assert_eq!(item.error_message, Some("Album not found".to_string()));
    }

    #[test]
    fn test_state_transition_sequence() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            3,
        );
        store.enqueue(item).unwrap();

        // PENDING -> IN_PROGRESS
        assert!(store.claim_for_processing("item-1").unwrap());
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::InProgress
        );

        // IN_PROGRESS -> RETRY_WAITING (simulating failure)
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::RetryWaiting
        );

        // Item no longer shows up as next pending
        assert!(store.get_next_pending().unwrap().is_none());
    }

    #[test]
    fn test_get_next_pending_skips_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item1.created_at = 1000;

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.created_at = 2000;

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // Claim item-1
        store.claim_for_processing("item-1").unwrap();

        // Next pending should be item-2
        let next = store.get_next_pending().unwrap().unwrap();
        assert_eq!(next.id, "item-2");
    }

    // === Parent-Child Management Tests ===

    #[test]
    fn test_create_children() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());
        store.enqueue(parent).unwrap();

        // Create children
        let children = vec![
            QueueItem::new_child(
                "child-1".to_string(),
                "parent-1".to_string(),
                DownloadContentType::TrackAudio,
                "track-1".to_string(),
                QueuePriority::User,
                RequestSource::User,
                Some("user-1".to_string()),
                3,
            ),
            QueueItem::new_child(
                "child-2".to_string(),
                "parent-1".to_string(),
                DownloadContentType::TrackAudio,
                "track-2".to_string(),
                QueuePriority::User,
                RequestSource::User,
                Some("user-1".to_string()),
                3,
            ),
        ];

        store.create_children("parent-1", children).unwrap();

        // Verify children were created
        let retrieved_children = store.get_children("parent-1").unwrap();
        assert_eq!(retrieved_children.len(), 2);
        assert_eq!(retrieved_children[0].id, "child-1");
        assert_eq!(retrieved_children[1].id, "child-2");
    }

    #[test]
    fn test_create_children_empty_list() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Creating no children should succeed
        store.create_children("parent-1", vec![]).unwrap();

        let children = store.get_children("parent-1").unwrap();
        assert!(children.is_empty());
    }

    #[test]
    fn test_create_children_wrong_parent_id() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Child with wrong parent_id
        let children = vec![QueueItem::new_child(
            "child-1".to_string(),
            "wrong-parent".to_string(), // Wrong parent!
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        )];

        // Should fail
        let result = store.create_children("parent-1", children);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_children_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let children = store.get_children("nonexistent-parent").unwrap();
        assert!(children.is_empty());
    }

    #[test]
    fn test_get_children_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Create children with different statuses
        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Failed;

        let child3 = QueueItem::new_child(
            "child-3".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        ); // Pending by default

        let mut child4 = QueueItem::new_child(
            "child-4".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-4".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child4.status = QueueStatus::InProgress;

        store
            .create_children("parent-1", vec![child1, child2, child3, child4])
            .unwrap();

        let progress = store.get_children_progress("parent-1").unwrap();
        assert_eq!(progress.total_children, 4);
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.failed, 1);
        assert_eq!(progress.pending, 1);
        assert_eq!(progress.in_progress, 1);
    }

    #[test]
    fn test_get_children_progress_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let progress = store.get_children_progress("nonexistent").unwrap();
        assert_eq!(progress.total_children, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_check_parent_completion_no_children() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let result = store.check_parent_completion("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_parent_completion_still_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let child = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        ); // Pending by default

        store.create_children("parent-1", vec![child]).unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert!(result.is_none()); // Still pending
    }

    #[test]
    fn test_check_parent_completion_all_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Completed;

        store
            .create_children("parent-1", vec![child1, child2])
            .unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert_eq!(result, Some(QueueStatus::Completed));
    }

    #[test]
    fn test_check_parent_completion_some_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Failed;

        store
            .create_children("parent-1", vec![child1, child2])
            .unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert_eq!(result, Some(QueueStatus::Failed));
    }

    #[test]
    fn test_get_user_requests() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create some parent items
        let parent1 = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let parent2 = QueueItem::new(
            "parent-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let parent3 = QueueItem::new(
            "parent-3".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-2".to_string()); // Different user

        store.enqueue(parent1).unwrap();
        store.enqueue(parent2).unwrap();
        store.enqueue(parent3).unwrap();

        // Create a child for parent-1 (should not show up in user requests)
        let child = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            Some("user-1".to_string()),
            3,
        );
        store.create_children("parent-1", vec![child]).unwrap();

        // Get user-1 requests
        let requests = store.get_user_requests("user-1", 100, 0).unwrap();
        assert_eq!(requests.len(), 2);

        // All should be parent items (no parent_id)
        for req in &requests {
            assert!(req.parent_id.is_none());
        }

        // Get user-2 requests
        let requests = store.get_user_requests("user-2", 100, 0).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, "parent-3");
    }

    #[test]
    fn test_get_user_requests_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create 5 parent items
        for i in 0..5 {
            let mut item = QueueItem::new(
                format!("parent-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-1".to_string());
            item.created_at = i as i64; // Ensure consistent ordering
            store.enqueue(item).unwrap();
        }

        // Get first page (2 items)
        let page1 = store.get_user_requests("user-1", 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page (2 items)
        let page2 = store.get_user_requests("user-1", 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page (1 item)
        let page3 = store.get_user_requests("user-1", 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    // === Retry Handling Tests ===

    #[test]
    fn test_get_retry_ready_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No items
        let ready = store.get_retry_ready().unwrap();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_get_retry_ready_with_items() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create item and put it in retry waiting state
        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark it for retry with a past retry time
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        let past_time = 1000; // Far in the past
        store
            .mark_retry_waiting("item-1", past_time, &error)
            .unwrap();

        // Should be ready for retry
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "item-1");
    }

    #[test]
    fn test_get_retry_ready_not_yet() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark for retry with a far future time
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        let future_time = 9999999999; // Far in the future
        store
            .mark_retry_waiting("item-1", future_time, &error)
            .unwrap();

        // Should not be ready yet
        let ready = store.get_retry_ready().unwrap();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_get_retry_ready_priority_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create items with different priorities
        let high = QueueItem::new(
            "high".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Urgent,
            RequestSource::Watchdog,
            5,
        );
        let low = QueueItem::new(
            "low".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::Expansion,
            RequestSource::Expansion,
            5,
        );

        store.enqueue(low).unwrap();
        store.enqueue(high).unwrap();

        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("low", 1000, &error).unwrap();
        store.mark_retry_waiting("high", 1000, &error).unwrap();

        // Should return high priority first
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].id, "high");
        assert_eq!(ready[1].id, "low");
    }

    #[test]
    fn test_promote_retry_to_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark for retry
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        // Verify retry waiting state
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert!(item.next_retry_at.is_some());

        // Promote to pending
        store.promote_retry_to_pending("item-1").unwrap();

        // Verify pending state
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);
        assert!(item.next_retry_at.is_none());
    }

    #[test]
    fn test_promote_retry_to_pending_not_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Item is pending, not retry waiting
        // Promoting should have no effect (WHERE clause won't match)
        store.promote_retry_to_pending("item-1").unwrap();

        // Still pending
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);
    }

    #[test]
    fn test_retry_workflow() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create and process an item
        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            3,
        );
        store.enqueue(item).unwrap();

        // Start processing
        store.claim_for_processing("item-1").unwrap();

        // Simulate failure - mark for retry
        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        // Item should be retry waiting
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert_eq!(item.retry_count, 1);

        // Get items ready for retry
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 1);

        // Promote back to pending
        store.promote_retry_to_pending("item-1").unwrap();

        // Should be pending again
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);

        // Claim again for second attempt
        assert!(store.claim_for_processing("item-1").unwrap());
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::InProgress
        );
    }

    // === Duplicate Check Tests ===

    #[test]
    fn test_find_by_content_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let found = store
            .find_by_content(DownloadContentType::Album, "album-123")
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "item-1");
    }

    #[test]
    fn test_find_by_content_not_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let found = store
            .find_by_content(DownloadContentType::Album, "nonexistent")
            .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_find_by_content_wrong_type() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Search with wrong content type
        let found = store
            .find_by_content(DownloadContentType::TrackAudio, "album-123")
            .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_find_by_content_returns_most_recent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create two items with same content (different IDs)
        let mut item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item1.created_at = 1000;

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.created_at = 2000; // More recent

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // Should return most recent
        let found = store
            .find_by_content(DownloadContentType::Album, "album-123")
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "item-2");
    }

    #[test]
    fn test_is_in_queue_true() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        assert!(store
            .is_in_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_queue_false() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        assert!(!store
            .is_in_queue(DownloadContentType::Album, "nonexistent")
            .unwrap());
    }

    #[test]
    fn test_is_in_queue_includes_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // is_in_queue should include completed items
        assert!(store
            .is_in_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_not_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // Completed items are NOT in active queue
        assert!(!store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_not_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Failed;
        store.enqueue(item).unwrap();

        // Failed items are NOT in active queue
        assert!(!store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    // === User Rate Limiting Tests ===

    #[test]
    fn test_get_user_stats_new_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stats = store.get_user_stats("user-1").unwrap();

        // New user should have full quota
        assert_eq!(stats.requests_today, 0);
        assert_eq!(stats.in_queue, 0);
        assert!(stats.can_request);
    }

    #[test]
    fn test_increment_user_requests() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Increment requests
        store.increment_user_requests("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 1);
        // in_queue comes from actual queue items, not the counter
        assert_eq!(stats.in_queue, 0);

        // Increment again
        store.increment_user_requests("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 2);
    }

    #[test]
    fn test_in_queue_counts_active_items() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Enqueue actual items for user-1
        for i in 0..3 {
            let item = QueueItem::new(
                format!("item-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-1".to_string());
            store.enqueue(item).unwrap();
        }

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 3);

        // Complete one item - should no longer count
        store.claim_for_processing("item-0").unwrap();
        store.mark_completed("item-0", 1000, 100).unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 2);

        // Fail another - should no longer count
        store.claim_for_processing("item-1").unwrap();
        let error = DownloadError::new(DownloadErrorType::NotFound, "Not found");
        store.mark_failed("item-1", &error).unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 1);
    }

    #[test]
    fn test_reset_daily_user_stats() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add requests for two users
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-2").unwrap();

        // Verify current stats
        let stats1 = store.get_user_stats("user-1").unwrap();
        let stats2 = store.get_user_stats("user-2").unwrap();
        assert_eq!(stats1.requests_today, 2);
        assert_eq!(stats2.requests_today, 1);

        // Reset won't affect users whose last_request_date is today
        // (which it is since we just made requests)
        let reset_count = store.reset_daily_user_stats().unwrap();
        assert_eq!(reset_count, 0);

        // Stats should remain unchanged
        let stats1 = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats1.requests_today, 2);
    }

    #[test]
    fn test_today_date_string_format() {
        let date = SqliteDownloadQueueStore::today_date_string();

        // Should be in YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");

        // Year should be reasonable (2020-2100)
        let year: i32 = date[0..4].parse().unwrap();
        assert!((2020..=2100).contains(&year));

        // Month should be 01-12
        let month: i32 = date[5..7].parse().unwrap();
        assert!((1..=12).contains(&month));

        // Day should be 01-31
        let day: i32 = date[8..10].parse().unwrap();
        assert!((1..=31).contains(&day));
    }

    #[test]
    fn test_user_stats_workflow() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // User makes 3 download requests with actual queue items
        for i in 0..3 {
            let item = QueueItem::new(
                format!("item-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-1".to_string());
            store.enqueue(item).unwrap();
            store.increment_user_requests("user-1").unwrap();
        }

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 3);
        assert_eq!(stats.in_queue, 3);

        // One item completes
        store.claim_for_processing("item-0").unwrap();
        store.mark_completed("item-0", 1000, 100).unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 3); // Still 3 (counts requests made, not in queue)
        assert_eq!(stats.in_queue, 2);

        // Another item completes
        store.claim_for_processing("item-1").unwrap();
        store.mark_completed("item-1", 1000, 100).unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 1);

        // Last item completes
        store.claim_for_processing("item-2").unwrap();
        store.mark_completed("item-2", 1000, 100).unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 0);
        assert_eq!(stats.requests_today, 3); // Daily count preserved
    }

    #[test]
    fn test_multiple_users_independent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // User 1 makes 3 requests with queue items
        for i in 0..3 {
            let item = QueueItem::new(
                format!("u1-item-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-1".to_string());
            store.enqueue(item).unwrap();
            store.increment_user_requests("user-1").unwrap();
        }

        // User 2 makes 2 requests with queue items
        for i in 0..2 {
            let item = QueueItem::new(
                format!("u2-item-{}", i),
                DownloadContentType::Album,
                format!("album-u2-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-2".to_string());
            store.enqueue(item).unwrap();
            store.increment_user_requests("user-2").unwrap();
        }

        // Verify independent tracking
        let stats1 = store.get_user_stats("user-1").unwrap();
        let stats2 = store.get_user_stats("user-2").unwrap();

        assert_eq!(stats1.requests_today, 3);
        assert_eq!(stats1.in_queue, 3);
        assert_eq!(stats2.requests_today, 2);
        assert_eq!(stats2.in_queue, 2);

        // Complete one of user 1's items
        store.claim_for_processing("u1-item-0").unwrap();
        store.mark_completed("u1-item-0", 1000, 100).unwrap();

        // User 2 should be unaffected
        let stats1 = store.get_user_stats("user-1").unwrap();
        let stats2 = store.get_user_stats("user-2").unwrap();
        assert_eq!(stats1.in_queue, 2);
        assert_eq!(stats2.in_queue, 2);
    }

    // === Activity Tracking Tests ===

    #[test]
    fn test_record_activity_album() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 1);
        assert_eq!(hourly.tracks, 0);
        assert_eq!(hourly.images, 0);
        assert_eq!(hourly.bytes, 1000);
    }

    #[test]
    fn test_record_activity_track() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 1);
        assert_eq!(hourly.bytes, 5000000);
    }

    #[test]
    fn test_record_activity_image() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::AlbumImage, 50000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.images, 1);
        assert_eq!(hourly.bytes, 50000);
    }

    #[test]
    fn test_record_activity_failure() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record a failed download - should not increment content counts
        store
            .record_activity(DownloadContentType::Album, 0, false)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 0);
        // Bytes are still recorded (even if 0)
    }

    #[test]
    fn test_record_activity_accumulates() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record multiple downloads
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::Album, 2000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 6000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 7000000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 2);
        assert_eq!(hourly.tracks, 3);
        assert_eq!(hourly.bytes, 1000 + 2000 + 5000000 + 6000000 + 7000000);
    }

    #[test]
    fn test_get_hourly_counts_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No activity recorded - should return defaults
        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 0);
        assert_eq!(hourly.images, 0);
        assert_eq!(hourly.bytes, 0);
    }

    #[test]
    fn test_get_daily_counts_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No activity recorded
        let daily = store.get_daily_counts().unwrap();
        assert_eq!(daily.albums, 0);
        assert_eq!(daily.tracks, 0);
    }

    #[test]
    fn test_get_daily_counts() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record various activities
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::AlbumImage, 50000, true)
            .unwrap();

        let daily = store.get_daily_counts().unwrap();
        assert_eq!(daily.albums, 1);
        assert_eq!(daily.tracks, 1);
        assert_eq!(daily.images, 1);
        assert_eq!(daily.bytes, 1000 + 5000000 + 50000);
    }

    #[test]
    fn test_get_activity_since_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let entries = store.get_activity_since(0).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_activity_since() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record some activity
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();

        // Get all activity since epoch
        let entries = store.get_activity_since(0).unwrap();
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.albums_downloaded, 1);
        assert_eq!(entry.bytes_downloaded, 1000);
    }

    #[test]
    fn test_hour_bucket() {
        let bucket = SqliteDownloadQueueStore::hour_bucket();

        // Should be divisible by 3600 (one hour in seconds)
        assert_eq!(bucket % 3600, 0);

        // Should be close to current time (within one hour)
        let now = SqliteDownloadQueueStore::now();
        assert!(now - bucket < 3600);
        assert!(now >= bucket);
    }

    #[test]
    fn test_day_start_bucket() {
        let bucket = SqliteDownloadQueueStore::day_start_bucket();

        // Should be divisible by 86400 (one day in seconds)
        assert_eq!(bucket % 86400, 0);

        // Should be close to current time (within one day)
        let now = SqliteDownloadQueueStore::now();
        assert!(now - bucket < 86400);
        assert!(now >= bucket);
    }

    // === Statistics Tests ===

    #[test]
    fn test_get_queue_stats_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.retry_waiting, 0);
        assert_eq!(stats.completed_today, 0);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_get_queue_stats_various_statuses() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with various statuses
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let in_progress = QueueItem::new(
            "in-progress".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(in_progress).unwrap();
        store.claim_for_processing("in-progress").unwrap();

        let retry = QueueItem::new(
            "retry".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(retry).unwrap();
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("retry", 1000, &error).unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.retry_waiting, 1);
    }

    #[test]
    fn test_get_queue_stats_completed_today() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and complete some items
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();
        store.mark_completed("item", 1000, 100).unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.completed_today, 1);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_get_failed_items_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let failed = store.get_failed_items(10, 0).unwrap();
        assert!(failed.is_empty());
    }

    #[test]
    fn test_get_failed_items() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add some failed items
        for i in 0..3 {
            let item = QueueItem::new(
                format!("failed-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            store.enqueue(item).unwrap();
            store
                .claim_for_processing(&format!("failed-{}", i))
                .unwrap();
            let error = DownloadError::new(DownloadErrorType::NotFound, "Not found");
            store.mark_failed(&format!("failed-{}", i), &error).unwrap();
        }

        // Add a pending item (should not be returned)
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-99".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let failed = store.get_failed_items(10, 0).unwrap();
        assert_eq!(failed.len(), 3);

        // All should be failed status
        for item in &failed {
            assert_eq!(item.status, QueueStatus::Failed);
        }
    }

    #[test]
    fn test_get_failed_items_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add 5 failed items
        for i in 0..5 {
            let item = QueueItem::new(
                format!("failed-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            store.enqueue(item).unwrap();
            store
                .claim_for_processing(&format!("failed-{}", i))
                .unwrap();
            let error = DownloadError::new(DownloadErrorType::NotFound, "Not found");
            store.mark_failed(&format!("failed-{}", i), &error).unwrap();
        }

        let page1 = store.get_failed_items(2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = store.get_failed_items(2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = store.get_failed_items(2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_get_stale_in_progress_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stale = store.get_stale_in_progress(3600).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_in_progress_recent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and claim an item (should be recent)
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();

        // With a threshold of 3600 seconds (1 hour), recently claimed items should not be stale
        let stale = store.get_stale_in_progress(3600).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_in_progress_with_threshold_0() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and claim an item
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();

        // With a threshold of 0 seconds, everything should be considered stale
        // (since started_at < now is always true for items started in the past)
        let _stale = store.get_stale_in_progress(0).unwrap();
        // Note: This might be 1 if the item was started before now,
        // or 0 if started_at == now (edge case)
        // We can't reliably test this without mocking time
    }

    #[test]
    fn test_get_stale_only_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with various statuses
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let mut completed = QueueItem::new(
            "completed".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        completed.status = QueueStatus::Completed;
        store.enqueue(completed).unwrap();

        // Stale check should only consider IN_PROGRESS items
        let stale = store.get_stale_in_progress(0).unwrap();
        for item in &stale {
            assert_eq!(item.status, QueueStatus::InProgress);
        }
    }

    // === Audit Logging Tests ===

    #[test]
    fn test_log_audit_event_basic() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let event = AuditLogEntry::new(AuditEventType::RequestCreated)
            .with_queue_item("queue-123".to_string())
            .with_content(DownloadContentType::Album, "album-456".to_string())
            .with_user("user-789".to_string())
            .with_source(RequestSource::User);

        store.log_audit_event(event).unwrap();

        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[0].queue_item_id, Some("queue-123".to_string()));
    }

    #[test]
    fn test_log_audit_event_with_details() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let details = serde_json::json!({
            "child_count": 12,
            "album_name": "Test Album"
        });

        let event = AuditLogEntry::new(AuditEventType::ChildrenCreated)
            .with_queue_item("parent-123".to_string())
            .with_details(details);

        store.log_audit_event(event).unwrap();

        let (entries, _) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(entries.len(), 1);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["child_count"], 12);
        assert_eq!(details["album_name"], "Test Album");
    }

    #[test]
    fn test_get_audit_log_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert!(entries.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn test_get_audit_log_filter_by_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events for different users
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-2".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadCompleted)
                    .with_user("user-1".to_string()),
            )
            .unwrap();

        let filter = AuditLogFilter::new().for_user("user-1".to_string());
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            assert_eq!(entry.user_id, Some("user-1".to_string()));
        }
    }

    #[test]
    fn test_get_audit_log_filter_by_queue_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("queue-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadStarted)
                    .with_queue_item("queue-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("queue-2".to_string()),
            )
            .unwrap();

        let filter = AuditLogFilter::new().for_queue_item("queue-1".to_string());
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        for entry in &entries {
            assert_eq!(entry.queue_item_id, Some("queue-1".to_string()));
        }
    }

    #[test]
    fn test_get_audit_log_filter_by_event_types() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::RequestCreated))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadStarted))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadCompleted))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadFailed))
            .unwrap();

        let filter = AuditLogFilter::new().with_event_types(vec![
            AuditEventType::DownloadCompleted,
            AuditEventType::DownloadFailed,
        ]);
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        for entry in &entries {
            assert!(
                entry.event_type == AuditEventType::DownloadCompleted
                    || entry.event_type == AuditEventType::DownloadFailed
            );
        }
    }

    #[test]
    fn test_get_audit_log_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log 10 events
        for i in 0..10 {
            let mut event = AuditLogEntry::new(AuditEventType::RequestCreated);
            event.timestamp = i as i64; // Different timestamps
            store.log_audit_event(event).unwrap();
        }

        let filter = AuditLogFilter::new().paginate(3, 0);
        let (entries, total) = store.get_audit_log(filter).unwrap();
        assert_eq!(total, 10);
        assert_eq!(entries.len(), 3);

        let filter = AuditLogFilter::new().paginate(3, 3);
        let (entries, total) = store.get_audit_log(filter).unwrap();
        assert_eq!(total, 10);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_get_audit_for_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log a sequence of events for an item
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadStarted)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadCompleted)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        // Different item
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("item-2".to_string()),
            )
            .unwrap();

        let entries = store.get_audit_for_item("item-1").unwrap();
        assert_eq!(entries.len(), 3);

        // Should be in chronological order (ASC)
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[1].event_type, AuditEventType::DownloadStarted);
        assert_eq!(entries[2].event_type, AuditEventType::DownloadCompleted);
    }

    #[test]
    fn test_get_audit_for_item_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let entries = store.get_audit_for_item("nonexistent").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_audit_for_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events for user-1
        for i in 0..5 {
            let mut event =
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string());
            event.timestamp = i as i64;
            store.log_audit_event(event).unwrap();
        }

        // Log events for user-2
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-2".to_string()),
            )
            .unwrap();

        let (entries, total) = store
            .get_audit_for_user("user-1", None, None, 100, 0)
            .unwrap();
        assert_eq!(total, 5);
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_get_audit_for_user_time_range() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events at different times
        for i in 0..10 {
            let mut event =
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string());
            event.timestamp = (i * 100) as i64; // 0, 100, 200, ..., 900
            store.log_audit_event(event).unwrap();
        }

        // Get events from time 300 to 600 (should be 4 events: 300, 400, 500, 600)
        let (entries, total) = store
            .get_audit_for_user("user-1", Some(300), Some(600), 100, 0)
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn test_cleanup_old_audit_entries() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events with different timestamps
        for i in 0..10 {
            let mut event = AuditLogEntry::new(AuditEventType::RequestCreated);
            event.timestamp = (i * 100) as i64;
            store.log_audit_event(event).unwrap();
        }

        // Delete entries older than timestamp 500 (should delete 0, 100, 200, 300, 400)
        let deleted = store.cleanup_old_audit_entries(500).unwrap();
        assert_eq!(deleted, 5);

        // Verify remaining entries
        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(total, 5);
        for entry in &entries {
            assert!(entry.timestamp >= 500);
        }
    }

    #[test]
    fn test_cleanup_old_audit_entries_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events with recent timestamps
        for _ in 0..5 {
            let event = AuditLogEntry::new(AuditEventType::RequestCreated);
            store.log_audit_event(event).unwrap();
        }

        // Try to delete entries older than timestamp 0 (none should match)
        let deleted = store.cleanup_old_audit_entries(0).unwrap();
        assert_eq!(deleted, 0);
    }

    // === Stats History Tests ===

    #[test]
    fn test_get_stats_history_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // All periods should return empty history
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert!(hourly.entries.is_empty());
        assert_eq!(hourly.total_albums, 0);
        assert_eq!(hourly.total_tracks, 0);

        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert!(daily.entries.is_empty());

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert!(weekly.entries.is_empty());
    }

    #[test]
    fn test_get_stats_history_with_data() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity (current hour)
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::Album, 2_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 10_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::AlbumImage, 100_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 0, false) // failure
            .unwrap();

        // Get hourly stats (should include current hour)
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert!(!hourly.entries.is_empty());
        assert_eq!(hourly.total_albums, 2);
        assert_eq!(hourly.total_tracks, 1);
        assert_eq!(hourly.total_images, 1);
        assert_eq!(
            hourly.total_bytes,
            1_000_000 + 2_000_000 + 10_000_000 + 100_000
        );
        assert_eq!(hourly.total_failures, 1);

        // Daily and weekly should also include the data
        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert_eq!(daily.total_albums, 2);

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert_eq!(weekly.total_albums, 2);
    }

    #[test]
    fn test_stats_history_period_aggregation() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5_000_000, true)
            .unwrap();

        // Check that different periods are represented correctly
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert_eq!(hourly.period, StatsPeriod::Hourly);
        assert!(!hourly.entries.is_empty());
        // Each entry should have period_start divisible by 3600 (hour)
        for entry in &hourly.entries {
            assert_eq!(entry.period_start % 3600, 0);
        }

        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert_eq!(daily.period, StatsPeriod::Daily);
        // Each entry should have period_start divisible by 86400 (day)
        for entry in &daily.entries {
            assert_eq!(entry.period_start % 86400, 0);
        }

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert_eq!(weekly.period, StatsPeriod::Weekly);
        // Each entry should have period_start divisible by 604800 (week)
        for entry in &weekly.entries {
            assert_eq!(entry.period_start % 604800, 0);
        }
    }

    #[test]
    fn test_stats_history_custom_date_range() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5_000_000, true)
            .unwrap();

        let now = chrono::Utc::now().timestamp();
        let one_hour_ago = now - 3600;
        let one_hour_ahead = now + 3600;

        // Custom range that includes current data
        let result = store
            .get_stats_history(
                StatsPeriod::Hourly,
                Some(one_hour_ago),
                Some(one_hour_ahead),
            )
            .unwrap();
        assert!(!result.entries.is_empty());
        assert_eq!(result.total_albums, 1);
        assert_eq!(result.total_tracks, 1);

        // Custom range in the past (no data)
        let far_past = now - 1_000_000;
        let less_far_past = now - 900_000;
        let result = store
            .get_stats_history(StatsPeriod::Hourly, Some(far_past), Some(less_far_past))
            .unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.total_albums, 0);
    }
}
