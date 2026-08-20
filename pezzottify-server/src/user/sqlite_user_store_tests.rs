#[cfg(test)]
mod tests {

    use super::*;
    use chrono;
    use tempfile::TempDir;

    fn create_tmp_store() -> (SqliteUserStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test.db");
        let store =
            SqliteUserStore::new(&temp_file_path, &crate::backup::DbRegistry::new()).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_create_user() {
        let (store, _temp_dir) = create_tmp_store();

        let user_id = store.create_user("test_user").unwrap();
        assert_eq!(user_id, 1);

        let duplicate_id = store.create_user("test_user");
        assert!(duplicate_id.is_err());
    }

    #[test]
    fn test_cannot_create_linked_content_without_user() {
        let (store, _temp_dir) = create_tmp_store();

        let result = store.set_user_liked_content(1, "test_content", LikedContentType::Album, true);
        assert!(result.is_err());
    }

    #[test]
    fn creates_liked_content() {
        let (store, _temp_dir) = create_tmp_store();

        let test_user_id = store.create_user("test_user").unwrap();
        store
            .set_user_liked_content(test_user_id, "test_content", LikedContentType::Artist, true)
            .unwrap();

        assert!(store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap()
            .unwrap());

        store
            .set_user_liked_content(test_user_id, "test_content", LikedContentType::Album, false)
            .unwrap();

        assert!(!store
            .is_user_liked_content(test_user_id, "test_content")
            .unwrap()
            .unwrap());
    }

    #[test]
    fn handles_playlists() {
        // First create a user
        let (store, _temp_dir) = create_tmp_store();
        let user_handle = "test_handle";
        let test_user_id = store.create_user(user_handle).unwrap();

        // Create a playlist
        let plyalist_id = store
            .create_user_playlist(
                test_user_id,
                "test_playlist",
                test_user_id,
                vec!["track1".to_string(), "track2".to_string()],
            )
            .unwrap();

        let user_playslits_ids = store.get_user_playlists(test_user_id).unwrap();
        assert_eq!(user_playslits_ids, vec![plyalist_id.clone()]);

        let playlist2_id = store
            .create_user_playlist(
                test_user_id,
                "test_playlist2",
                test_user_id,
                vec!["track1".to_string(), "track2".to_string()],
            )
            .unwrap();

        let user_playslits_ids = store.get_user_playlists(test_user_id).unwrap();

        assert_eq!(
            user_playslits_ids,
            vec![plyalist_id.clone(), playlist2_id.clone()]
        );

        store
            .delete_user_playlist(&plyalist_id, test_user_id)
            .unwrap();
        store
            .delete_user_playlist(&playlist2_id, test_user_id)
            .unwrap();

        assert_eq!(store.get_user_playlists(test_user_id).unwrap().len(), 0,);
    }

    #[test]
    fn test_migration_v3_to_v4() {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test_migration.db");

        // Create a V3 database manually
        {
            let conn = Connection::open(&temp_file_path).unwrap();
            VERSIONED_SCHEMAS[3].create(&conn).unwrap(); // V3 is at index 3

            // Add some test data
            conn.execute(
                "INSERT INTO user (handle) VALUES (?1)",
                params!["test_user"],
            )
            .unwrap();
            let user_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO liked_content (user_id, content_id, content_type) VALUES (?1, ?2, ?3)",
                params![user_id, "test_content_id", 1],
            )
            .unwrap();

            conn.execute(
                "INSERT INTO user_playlist (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
                params!["playlist123", user_id, "Test Playlist", user_id],
            )
            .unwrap();

            // Verify we're at V3
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(db_version, BASE_DB_VERSION as i64 + 3);
        }

        // Now open with SqliteUserStore, which should trigger migration to latest
        let store =
            SqliteUserStore::new(&temp_file_path, &crate::backup::DbRegistry::new()).unwrap();

        // Verify we're now at the latest version
        {
            let conn = store.conn.lock().unwrap();
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                db_version,
                BASE_DB_VERSION as i64 + VERSIONED_SCHEMAS.last().unwrap().version as i64
            );

            // Verify new tables exist
            let user_role_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_role'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(user_role_table_exists, 1);

            let user_extra_permission_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_extra_permission'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(user_extra_permission_table_exists, 1);

            // Verify indices exist with correct names
            let role_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_user_role_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(role_index_exists, 1);

            let permission_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_user_extra_permission_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(permission_index_exists, 1);

            // Verify listening_events table exists (V6)
            let listening_events_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='listening_events'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(listening_events_table_exists, 1);

            // Verify listening_events indices exist
            let listening_events_user_id_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_listening_events_user_id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(listening_events_user_id_index_exists, 1);
        }

        // Verify old data is still intact
        let user_id = store.get_user_id("test_user").unwrap().unwrap();
        assert_eq!(user_id, 1);

        let liked_content = store
            .is_user_liked_content(user_id, "test_content_id")
            .unwrap()
            .unwrap();
        assert!(liked_content);

        let playlists = store.get_user_playlists(user_id).unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0], "playlist123");

        // Test new permission functionality
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);

        // Test adding extra permission
        let grant = PermissionGrant::Extra {
            start_time: SystemTime::now(),
            end_time: None,
            permission: Permission::EditCatalog,
            countdown: None,
        };
        let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
        assert!(permission_id > 0);

        // Test resolving permissions
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog)); // From Regular role
        assert!(permissions.contains(&Permission::EditCatalog)); // From extra permission
    }

    #[test]
    fn test_migration_v7_to_v8_device_table() {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test_migration_v7_v8.db");

        // Create a V7 database manually
        {
            let conn = Connection::open(&temp_file_path).unwrap();
            VERSIONED_SCHEMAS[7].create(&conn).unwrap(); // V7 is at index 7

            // Add a user and auth token (pre-migration, auth_token doesn't have device_id)
            conn.execute(
                "INSERT INTO user (handle) VALUES (?1)",
                params!["test_user"],
            )
            .unwrap();
            let user_id = conn.last_insert_rowid();

            // Insert a token (V7 auth_token doesn't have device_id)
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            conn.execute(
                "INSERT INTO auth_token (user_id, value, created) VALUES (?1, ?2, ?3)",
                params![user_id, "old-token-value", now],
            )
            .unwrap();

            // Verify we're at V7
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(db_version, BASE_DB_VERSION as i64 + 7);

            // Verify token exists before migration
            let token_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM auth_token", [], |row| row.get(0))
                .unwrap();
            assert_eq!(token_count, 1);
        }

        // Now open with SqliteUserStore, which should trigger migration to latest
        let store =
            SqliteUserStore::new(&temp_file_path, &crate::backup::DbRegistry::new()).unwrap();

        // Verify we're now at the latest version
        {
            let conn = store.conn.lock().unwrap();
            let db_version: i64 = conn
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                db_version,
                BASE_DB_VERSION as i64 + VERSIONED_SCHEMAS.last().unwrap().version as i64
            );

            // Verify device table exists
            let device_table_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='device'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(device_table_exists, 1);

            // Verify device table has expected columns
            let device_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(device)")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(device_columns.contains(&"id".to_string()));
            assert!(device_columns.contains(&"device_uuid".to_string()));
            assert!(device_columns.contains(&"user_id".to_string()));
            assert!(device_columns.contains(&"device_type".to_string()));
            assert!(device_columns.contains(&"device_name".to_string()));
            assert!(device_columns.contains(&"os_info".to_string()));
            assert!(device_columns.contains(&"first_seen".to_string()));
            assert!(device_columns.contains(&"last_seen".to_string()));

            // Verify auth_token has device_id column
            let auth_token_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(auth_token)")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(auth_token_columns.contains(&"device_id".to_string()));

            // Verify old tokens were deleted during migration
            let token_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM auth_token", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                token_count, 0,
                "Old tokens should be deleted during V8 migration"
            );

            // Verify device indices exist
            let device_uuid_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_device_uuid'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(device_uuid_index_exists, 1);

            let device_user_index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_device_user'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(device_user_index_exists, 1);
        }

        // Verify user data is still intact
        let user_id = store.get_user_id("test_user").unwrap().unwrap();
        assert_eq!(user_id, 1);

        // Test device functionality works after migration
        let reg = DeviceRegistration {
            device_uuid: "post-migration-device".to_string(),
            device_type: DeviceType::Android,
            device_name: Some("Test Phone".to_string()),
            os_info: Some("Android 14".to_string()),
        };
        let device_id = store.register_or_update_device(&reg).unwrap();
        assert!(device_id > 0);

        let device = store.get_device(device_id).unwrap().unwrap();
        assert_eq!(device.device_uuid, "post-migration-device");
        assert_eq!(device.device_type, DeviceType::Android);
    }

    #[test]
    fn test_migration_v13_to_v14_adds_event_idempotency_columns() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_migration_v13_v14.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            VERSIONED_SCHEMAS[13].create(&conn).unwrap();
            conn.execute("INSERT INTO user (handle) VALUES ('migration-user')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO user_events (user_id, event_type, payload)
                 VALUES (1, 'content_liked', '{\"type\":\"content_liked\",\"payload\":{\"content_type\":\"track\",\"content_id\":\"old-track\"}}')",
                [],
            )
            .unwrap();
        }

        let store = SqliteUserStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();
        let old_events = store.get_events_since(1, 0).unwrap();
        assert_eq!(old_events.len(), 1);
        assert_eq!(old_events[0].operation_id, None);

        let event = store
            .set_liked_content_with_event(
                1,
                "new-track",
                LikedContentType::Track,
                true,
                Some("migration-operation"),
            )
            .unwrap();
        assert_eq!(event.operation_id.as_deref(), Some("migration-operation"));
    }

    #[test]
    fn test_migration_v14_to_v15_rotates_plaintext_tokens() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_migration_v14_v15.db");
        let old_token = "plaintext-session-that-must-be-revoked";
        {
            let conn = Connection::open(&db_path).unwrap();
            VERSIONED_SCHEMAS[14].create(&conn).unwrap();
            conn.execute("INSERT INTO user (handle) VALUES ('migration-user')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO auth_token (user_id, value, created) VALUES (1, ?1, ?2)",
                params![old_token, unix_timestamp_now().unwrap()],
            )
            .unwrap();
        }

        let store = SqliteUserStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();

        assert!(store
            .get_user_auth_token(&AuthTokenValue(old_token.to_string()))
            .unwrap()
            .is_none());
        let conn = store.conn.lock().unwrap();
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(auth_token)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "value"));
        assert!(columns.iter().any(|column| column == "token_hash"));
        assert!(columns.iter().any(|column| column == "token_id"));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM auth_token", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_add_single_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add a single role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify the role was added
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_add_multiple_roles() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add multiple roles
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Verify both roles were added
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Regular));
        assert!(roles.contains(&UserRole::Admin));
    }

    #[test]
    fn test_add_duplicate_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add the same role twice
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify the role is only present once
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_remove_role_with_multiple_roles() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add multiple roles
        store.add_user_role(user_id, UserRole::Regular).unwrap();
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Remove one role
        store.remove_user_role(user_id, UserRole::Regular).unwrap();

        // Verify only Admin remains
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Admin);
    }

    #[test]
    fn test_remove_last_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add a single role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Remove the role
        store.remove_user_role(user_id, UserRole::Regular).unwrap();

        // Verify no roles remain
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 0);

        // Verify the database row was deleted
        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_remove_nonexistent_role() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add Regular role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Try to remove Admin role (not present)
        store.remove_user_role(user_id, UserRole::Admin).unwrap();

        // Verify Regular is still there
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], UserRole::Regular);
    }

    #[test]
    fn test_get_roles_with_comma_separated_string() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Manually insert comma-separated roles into the database
        let conn = store.conn.lock().unwrap();
        conn.execute(
            &format!(
                "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id, "Admin,Regular"],
        )
        .unwrap();
        drop(conn);

        // Verify both roles are parsed correctly
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Admin));
        assert!(roles.contains(&UserRole::Regular));
    }

    #[test]
    fn test_get_roles_with_spaces_in_comma_separated_string() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Manually insert comma-separated roles with spaces
        let conn = store.conn.lock().unwrap();
        conn.execute(
            &format!(
                "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id, "Admin, Regular"],
        )
        .unwrap();
        drop(conn);

        // Verify both roles are parsed correctly (spaces are trimmed)
        let roles = store.get_user_roles(user_id).unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&UserRole::Admin));
        assert!(roles.contains(&UserRole::Regular));
    }

    #[test]
    fn test_role_permissions_resolution() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Add Regular role
        store.add_user_role(user_id, UserRole::Regular).unwrap();

        // Verify Regular permissions
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog));
        assert!(permissions.contains(&Permission::LikeContent));
        assert!(permissions.contains(&Permission::OwnPlaylists));
        assert!(!permissions.contains(&Permission::EditCatalog));
        assert!(!permissions.contains(&Permission::ManagePermissions));

        // Add Admin role
        store.add_user_role(user_id, UserRole::Admin).unwrap();

        // Verify Admin permissions are now present
        let permissions = store.resolve_user_permissions(user_id).unwrap();
        assert!(permissions.contains(&Permission::AccessCatalog));
        assert!(permissions.contains(&Permission::LikeContent));
        assert!(permissions.contains(&Permission::OwnPlaylists));
        assert!(permissions.contains(&Permission::EditCatalog));
        assert!(permissions.contains(&Permission::ManagePermissions));
        assert!(permissions.contains(&Permission::ServerAdmin));
    }

    #[test]
    fn test_auth_token_last_used_update() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a token
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };

        store.add_user_auth_token(token.clone()).unwrap();

        // Verify last_used is initially None
        let retrieved_token = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert!(retrieved_token.last_used.is_none());

        // Update last_used timestamp
        store
            .update_user_auth_token_last_used_timestamp(&token.value)
            .unwrap();

        // Verify last_used is now set
        let updated_token = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert!(updated_token.last_used.is_some());
    }

    #[test]
    fn auth_token_storage_contains_digest_and_non_secret_id_only() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };

        store.add_user_auth_token(token.clone()).unwrap();

        let digest = auth_token_digest(&token.value);
        let conn = store.conn.lock().unwrap();
        let (stored_hash, stored_id): (String, String) = conn
            .query_row("SELECT token_hash, token_id FROM auth_token", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(stored_hash, digest);
        assert_eq!(stored_id, auth_token_identifier(&digest));
        assert_ne!(stored_hash, token.value.0);
        assert_ne!(stored_id, token.value.0);
        drop(conn);

        let listed = store.get_all_user_auth_tokens("test_user").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].value.0, auth_token_identifier(&digest));
        assert_ne!(listed[0].value, token.value);
    }

    #[test]
    fn auth_token_lookup_enforces_absolute_expiry_and_deletes_row() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(token.clone()).unwrap();

        let now = unix_timestamp_now().unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE auth_token SET created = ?1, last_used = ?2 WHERE token_hash = ?3",
            params![
                now - AUTH_TOKEN_ABSOLUTE_TTL_SECS - 1,
                now,
                auth_token_digest(&token.value)
            ],
        )
        .unwrap();
        drop(conn);

        assert!(store.get_user_auth_token(&token.value).unwrap().is_none());
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM auth_token", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn auth_token_lookup_enforces_idle_expiry_and_deletes_row() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(token.clone()).unwrap();

        let now = unix_timestamp_now().unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE auth_token SET last_used = ?1 WHERE token_hash = ?2",
            params![
                now - AUTH_TOKEN_IDLE_TTL_SECS - 1,
                auth_token_digest(&token.value)
            ],
        )
        .unwrap();
        drop(conn);

        assert!(store.get_user_auth_token(&token.value).unwrap().is_none());
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM auth_token", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn auth_token_expiry_boundaries_are_inclusive() {
        let now = 10 * AUTH_TOKEN_ABSOLUTE_TTL_SECS;
        assert!(!auth_token_is_expired(
            now - AUTH_TOKEN_ABSOLUTE_TTL_SECS + 1,
            Some(now),
            now
        ));
        assert!(auth_token_is_expired(
            now - AUTH_TOKEN_ABSOLUTE_TTL_SECS,
            Some(now),
            now
        ));
        assert!(!auth_token_is_expired(
            now,
            Some(now - AUTH_TOKEN_IDLE_TTL_SECS + 1),
            now
        ));
        assert!(auth_token_is_expired(
            now,
            Some(now - AUTH_TOKEN_IDLE_TTL_SECS),
            now
        ));
    }

    #[test]
    fn test_prune_unused_auth_tokens() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create an old token (simulate by manually inserting with old timestamp)
        let old_token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(old_token.clone()).unwrap();

        // Keep this inside the authoritative 7-day idle lifetime so lookup still
        // succeeds, then exercise a stricter caller-requested 5-day prune window.
        let conn = store.conn.lock().unwrap();
        let six_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (6 * 24 * 60 * 60);
        conn.execute(
            "UPDATE auth_token SET created = ?1 WHERE token_hash = ?2",
            params![six_days_ago as i64, auth_token_digest(&old_token.value)],
        )
        .unwrap();
        drop(conn);

        // Create a recent token
        let recent_token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(recent_token.clone()).unwrap();

        // Verify both tokens exist
        assert!(store
            .get_user_auth_token(&old_token.value)
            .unwrap()
            .is_some());
        assert!(store
            .get_user_auth_token(&recent_token.value)
            .unwrap()
            .is_some());

        // Prune tokens older than 5 days
        let pruned = store.prune_unused_auth_tokens(5).unwrap();
        assert_eq!(pruned, 1);

        // Verify old token is gone and recent token remains
        assert!(store
            .get_user_auth_token(&old_token.value)
            .unwrap()
            .is_none());
        assert!(store
            .get_user_auth_token(&recent_token.value)
            .unwrap()
            .is_some());
    }

    #[test]
    fn test_prune_respects_last_used() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create an old token
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        store.add_user_auth_token(token.clone()).unwrap();

        // Manually set the created timestamp to 10 days ago
        let conn = store.conn.lock().unwrap();
        let ten_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (10 * 24 * 60 * 60);
        conn.execute(
            "UPDATE auth_token SET created = ?1 WHERE token_hash = ?2",
            params![ten_days_ago as i64, auth_token_digest(&token.value)],
        )
        .unwrap();
        drop(conn);

        // Update last_used to now (recent usage)
        store
            .update_user_auth_token_last_used_timestamp(&token.value)
            .unwrap();

        // Prune tokens older than 7 days
        let pruned = store.prune_unused_auth_tokens(7).unwrap();
        assert_eq!(pruned, 0);

        // Verify token still exists because it was recently used
        assert!(store.get_user_auth_token(&token.value).unwrap().is_some());
    }

    // Bandwidth tracking tests

    #[test]
    fn test_record_bandwidth_usage() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record initial bandwidth usage
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();

        // Verify the record was created
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].user_id, user_id);
        assert_eq!(records[0].date, 20241127);
        assert_eq!(records[0].endpoint_category, "stream");
        assert_eq!(records[0].bytes_sent, 1024);
        assert_eq!(records[0].request_count, 1);
    }

    #[test]
    fn test_record_bandwidth_aggregates_same_day_category() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth usage twice for same day/category
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 2048, 2)
            .unwrap();

        // Verify values were aggregated (not duplicated)
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].bytes_sent, 3072); // 1024 + 2048
        assert_eq!(records[0].request_count, 3); // 1 + 2
    }

    #[test]
    fn test_record_bandwidth_different_categories() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth for different categories on same day
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "catalog", 512, 5)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "image", 2048, 2)
            .unwrap();

        // Verify separate records for each category
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_get_user_bandwidth_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth for different categories
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 10000, 10)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "catalog", 5000, 100)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241128, "stream", 15000, 15)
            .unwrap();

        // Get summary
        let summary = store
            .get_user_bandwidth_summary(user_id, 20241127, 20241128)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_bytes_sent, 30000); // 10000 + 5000 + 15000
        assert_eq!(summary.total_requests, 125); // 10 + 100 + 15

        // Check category breakdown
        let stream_stats = summary.by_category.get("stream").unwrap();
        assert_eq!(stream_stats.bytes_sent, 25000); // 10000 + 15000
        assert_eq!(stream_stats.request_count, 25); // 10 + 15

        let catalog_stats = summary.by_category.get("catalog").unwrap();
        assert_eq!(catalog_stats.bytes_sent, 5000);
        assert_eq!(catalog_stats.request_count, 100);
    }

    #[test]
    fn test_get_all_bandwidth_usage() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Record bandwidth for different users
        store
            .record_bandwidth_usage(user1_id, 20241127, "stream", 1000, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "catalog", 2000, 2)
            .unwrap();

        // Get all bandwidth usage
        let records = store.get_all_bandwidth_usage(20241127, 20241127).unwrap();

        assert_eq!(records.len(), 2);
        // Records should include both users
        let user_ids: Vec<usize> = records.iter().map(|r| r.user_id).collect();
        assert!(user_ids.contains(&user1_id));
        assert!(user_ids.contains(&user2_id));
    }

    #[test]
    fn test_get_total_bandwidth_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Record bandwidth for different users
        store
            .record_bandwidth_usage(user1_id, 20241127, "stream", 1000, 10)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "stream", 2000, 20)
            .unwrap();
        store
            .record_bandwidth_usage(user2_id, 20241127, "catalog", 500, 5)
            .unwrap();

        // Get total summary
        let summary = store
            .get_total_bandwidth_summary(20241127, 20241127)
            .unwrap();

        assert_eq!(summary.user_id, None); // Total summary has no specific user
        assert_eq!(summary.total_bytes_sent, 3500); // 1000 + 2000 + 500
        assert_eq!(summary.total_requests, 35); // 10 + 20 + 5
    }

    #[test]
    fn test_bandwidth_date_range_filter() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth on different days
        store
            .record_bandwidth_usage(user_id, 20241125, "stream", 1000, 1)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241126, "stream", 2000, 2)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 3000, 3)
            .unwrap();
        store
            .record_bandwidth_usage(user_id, 20241128, "stream", 4000, 4)
            .unwrap();

        // Query for subset of dates
        let records = store
            .get_user_bandwidth_usage(user_id, 20241126, 20241127)
            .unwrap();

        assert_eq!(records.len(), 2);
        let dates: Vec<u32> = records.iter().map(|r| r.date).collect();
        assert!(dates.contains(&20241126));
        assert!(dates.contains(&20241127));
        assert!(!dates.contains(&20241125));
        assert!(!dates.contains(&20241128));
    }

    #[test]
    fn test_bandwidth_usage_deleted_on_user_delete() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record bandwidth usage
        store
            .record_bandwidth_usage(user_id, 20241127, "stream", 1024, 1)
            .unwrap();

        // Verify record exists
        let records = store
            .get_user_bandwidth_usage(user_id, 20241127, 20241127)
            .unwrap();
        assert_eq!(records.len(), 1);

        // Delete user (bandwidth_usage has ON DELETE CASCADE foreign key)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify bandwidth records were deleted with user
        let all_records = store.get_all_bandwidth_usage(20241127, 20241127).unwrap();
        assert_eq!(all_records.len(), 0);
    }

    // ==================== Listening Events Tests ====================

    fn create_test_listening_event(user_id: usize, track_id: &str, date: u32) -> ListeningEvent {
        ListeningEvent {
            id: None,
            user_id,
            track_id: track_id.to_string(),
            session_id: None,
            started_at: 1732982400, // Some fixed timestamp
            ended_at: Some(1732982587),
            duration_seconds: 187,
            track_duration_seconds: 210,
            completed: true,
            seek_count: 2,
            pause_count: 1,
            playback_context: Some("album".to_string()),
            client_type: Some("android".to_string()),
            date,
        }
    }

    #[test]
    fn test_record_listening_event_basic() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = create_test_listening_event(user_id, "tra_12345", 20241201);
        let (id, created) = store.record_listening_event(event).unwrap();

        assert!(id > 0);
        assert!(created);
    }

    #[test]
    fn test_record_listening_event_update_with_session_id() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let mut event = create_test_listening_event(user_id, "tra_12345", 20241201);
        event.session_id = Some("unique-session-uuid".to_string());
        event.duration_seconds = 100;
        event.ended_at = None;

        // First insert (in-progress session)
        let (id1, created1) = store.record_listening_event(event.clone()).unwrap();
        assert!(id1 > 0);
        // created1 is false because ended_at is None (not finalized yet)
        assert!(!created1);

        // Second insert with same session_id but updated data (finalized session)
        event.duration_seconds = 300;
        event.ended_at = Some(1732982700);
        let (id2, created2) = store.record_listening_event(event.clone()).unwrap();
        assert_eq!(id2, id1);
        // Now created2 is true because ended_at is Some (finalized)
        assert!(created2);

        // A retry cannot rewrite an event after it has contributed to aggregates.
        event.duration_seconds = 42;
        event.ended_at = None;
        let (id3, created3) = store.record_listening_event(event).unwrap();
        assert_eq!(id3, id1);
        assert!(!created3);

        // Verify the data was updated
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration_seconds, 300);
        assert!(events[0].ended_at.is_some());
    }

    #[test]
    fn test_listening_session_cannot_switch_tracks() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let mut event = create_test_listening_event(user_id, "tra_original", 20241201);
        event.session_id = Some("stable-session-id".to_string());
        event.ended_at = None;
        let (id, created) = store.record_listening_event(event.clone()).unwrap();
        assert!(!created);

        event.track_id = "tra_replacement".to_string();
        event.ended_at = Some(1732982700);
        let (retry_id, retry_created) = store.record_listening_event(event).unwrap();
        assert_eq!(retry_id, id);
        assert!(!retry_created);

        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].track_id, "tra_original");
        assert!(events[0].ended_at.is_none());
    }

    #[test]
    fn test_record_listening_event_without_session_id_always_inserts() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = create_test_listening_event(user_id, "tra_12345", 20241201);

        // First insert
        let (id1, created1) = store.record_listening_event(event.clone()).unwrap();
        assert!(created1);

        // Second insert without session_id should create new record
        let (id2, created2) = store.record_listening_event(event).unwrap();
        assert!(created2);
        assert_ne!(id1, id2); // Different IDs
    }

    #[test]
    fn test_get_user_listening_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record events on different dates
        let event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        let event2 = create_test_listening_event(user_id, "tra_002", 20241202);
        let event3 = create_test_listening_event(user_id, "tra_003", 20241203);

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        // Get all events
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, None, None)
            .unwrap();
        assert_eq!(events.len(), 3);

        // Get events for specific date range
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241202, None, None)
            .unwrap();
        assert_eq!(events.len(), 2);

        // Test pagination
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, Some(2), None)
            .unwrap();
        assert_eq!(events.len(), 2);

        let events = store
            .get_user_listening_events(user_id, 20241201, 20241203, Some(2), Some(2))
            .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_get_user_listening_summary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record completed event
        let mut event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        event1.duration_seconds = 200;
        event1.completed = true;

        // Record incomplete event
        let mut event2 = create_test_listening_event(user_id, "tra_002", 20241201);
        event2.duration_seconds = 50;
        event2.completed = false;

        // Record another play of the same track
        let mut event3 = create_test_listening_event(user_id, "tra_001", 20241201);
        event3.duration_seconds = 180;
        event3.completed = true;

        // Raw progress events remain queryable but cannot affect trusted aggregates.
        let mut progress = create_test_listening_event(user_id, "tra_unfinished", 20241201);
        progress.ended_at = None;
        progress.duration_seconds = 999;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();
        store.record_listening_event(progress).unwrap();

        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241201)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_plays, 3);
        assert_eq!(summary.total_duration_seconds, 430); // 200 + 50 + 180
        assert_eq!(summary.completed_plays, 2);
        assert_eq!(summary.unique_tracks, 2); // tra_001 and tra_002
    }

    #[test]
    fn test_get_user_listening_history() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record events - tra_001 played twice, tra_002 played once
        let mut event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        event1.started_at = 1000;
        event1.duration_seconds = 100;

        let mut event2 = create_test_listening_event(user_id, "tra_002", 20241201);
        event2.started_at = 2000;
        event2.duration_seconds = 150;

        let mut event3 = create_test_listening_event(user_id, "tra_001", 20241201);
        event3.started_at = 3000;
        event3.duration_seconds = 120;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let history = store.get_user_listening_history(user_id, 10).unwrap();

        assert_eq!(history.len(), 2); // 2 unique tracks

        // Should be ordered by last_played_at descending
        assert_eq!(history[0].track_id, "tra_001");
        assert_eq!(history[0].play_count, 2);
        assert_eq!(history[0].total_duration_seconds, 220); // 100 + 120
        assert_eq!(history[0].last_played_at, 3000);

        assert_eq!(history[1].track_id, "tra_002");
        assert_eq!(history[1].play_count, 1);
        assert_eq!(history[1].total_duration_seconds, 150);
    }

    #[test]
    fn test_get_track_listening_stats() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // User 1 plays track twice
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.duration_seconds = 100;
        event1.completed = true;

        let mut event2 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event2.duration_seconds = 50;
        event2.completed = false;

        // User 2 plays track once
        let mut event3 = create_test_listening_event(user2_id, "tra_001", 20241201);
        event3.duration_seconds = 200;
        event3.completed = true;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let stats = store
            .get_track_listening_stats("tra_001", 20241201, 20241201)
            .unwrap();

        assert_eq!(stats.track_id, "tra_001");
        assert_eq!(stats.play_count, 3);
        assert_eq!(stats.total_duration_seconds, 350); // 100 + 50 + 200
        assert_eq!(stats.completed_count, 2);
        assert_eq!(stats.unique_listeners, 2);
    }

    #[test]
    fn test_get_daily_listening_stats() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // Day 1: user1 plays tra_001
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.duration_seconds = 100;
        event1.completed = true;

        // Day 1: user2 plays tra_002
        let mut event2 = create_test_listening_event(user2_id, "tra_002", 20241201);
        event2.duration_seconds = 150;
        event2.completed = false;

        // Day 2: user1 plays tra_001 again
        let mut event3 = create_test_listening_event(user1_id, "tra_001", 20241202);
        event3.duration_seconds = 200;
        event3.completed = true;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let daily_stats = store.get_daily_listening_stats(20241201, 20241202).unwrap();

        assert_eq!(daily_stats.len(), 2);

        // Day 1 stats
        let day1 = daily_stats.iter().find(|d| d.date == 20241201).unwrap();
        assert_eq!(day1.total_plays, 2);
        assert_eq!(day1.total_duration_seconds, 250); // 100 + 150
        assert_eq!(day1.completed_plays, 1);
        assert_eq!(day1.unique_users, 2);
        assert_eq!(day1.unique_tracks, 2);

        // Day 2 stats
        let day2 = daily_stats.iter().find(|d| d.date == 20241202).unwrap();
        assert_eq!(day2.total_plays, 1);
        assert_eq!(day2.total_duration_seconds, 200);
        assert_eq!(day2.completed_plays, 1);
        assert_eq!(day2.unique_users, 1);
        assert_eq!(day2.unique_tracks, 1);
    }

    #[test]
    fn test_get_top_tracks() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // tra_001: 3 plays
        for _ in 0..3 {
            let event = create_test_listening_event(user_id, "tra_001", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_002: 5 plays
        for _ in 0..5 {
            let event = create_test_listening_event(user_id, "tra_002", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_003: 1 play
        let event = create_test_listening_event(user_id, "tra_003", 20241201);
        store.record_listening_event(event).unwrap();

        let top_tracks = store.get_top_tracks(20241201, 20241201, 10).unwrap();

        assert_eq!(top_tracks.len(), 3);
        // Should be ordered by play_count descending
        assert_eq!(top_tracks[0].track_id, "tra_002");
        assert_eq!(top_tracks[0].play_count, 5);
        assert_eq!(top_tracks[1].track_id, "tra_001");
        assert_eq!(top_tracks[1].play_count, 3);
        assert_eq!(top_tracks[2].track_id, "tra_003");
        assert_eq!(top_tracks[2].play_count, 1);

        // Test limit
        let top_tracks = store.get_top_tracks(20241201, 20241201, 2).unwrap();
        assert_eq!(top_tracks.len(), 2);
    }

    #[test]
    fn test_get_all_track_play_counts() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // tra_001: 3 plays
        for _ in 0..3 {
            let event = create_test_listening_event(user_id, "tra_001", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_002: 5 plays
        for _ in 0..5 {
            let event = create_test_listening_event(user_id, "tra_002", 20241201);
            store.record_listening_event(event).unwrap();
        }

        // tra_003: 1 play
        let event = create_test_listening_event(user_id, "tra_003", 20241201);
        store.record_listening_event(event).unwrap();

        // Get all track play counts (no limit)
        let all_counts = store.get_all_track_play_counts(20241201, 20241201).unwrap();

        // Should return all 3 tracks
        assert_eq!(all_counts.len(), 3);

        // Verify play counts (order is not guaranteed since there's no ORDER BY)
        let tra_001 = all_counts.iter().find(|t| t.track_id == "tra_001").unwrap();
        assert_eq!(tra_001.play_count, 3);

        let tra_002 = all_counts.iter().find(|t| t.track_id == "tra_002").unwrap();
        assert_eq!(tra_002.play_count, 5);

        let tra_003 = all_counts.iter().find(|t| t.track_id == "tra_003").unwrap();
        assert_eq!(tra_003.play_count, 1);
    }

    #[test]
    fn test_get_all_track_play_counts_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let _user_id = store.create_user("test_user").unwrap();

        let all_counts = store.get_all_track_play_counts(20241201, 20241231).unwrap();
        assert!(all_counts.is_empty());
    }

    #[test]
    fn test_prune_listening_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get today's date in YYYYMMDD format
        let today = chrono::Utc::now();
        let today_date: u32 = today.format("%Y%m%d").to_string().parse().unwrap();

        // Calculate old date (60 days ago)
        let old_date = (today - chrono::Duration::days(60))
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap();

        // Calculate recent date (5 days ago)
        let recent_date = (today - chrono::Duration::days(5))
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap();

        // Record old event (60 days ago)
        let old_event = create_test_listening_event(user_id, "tra_old", old_date);
        store.record_listening_event(old_event).unwrap();

        // Record recent event (5 days ago)
        let recent_event = create_test_listening_event(user_id, "tra_recent", recent_date);
        store.record_listening_event(recent_event).unwrap();

        // Verify both exist
        let all_events = store
            .get_user_listening_events(user_id, old_date, today_date, None, None)
            .unwrap();
        assert_eq!(all_events.len(), 2);

        // Prune events older than 30 days (should delete the 60-day-old event)
        let pruned = store.prune_listening_events(30).unwrap();
        assert_eq!(pruned, 1);

        // Verify only recent event remains
        let remaining_events = store
            .get_user_listening_events(user_id, old_date, today_date, None, None)
            .unwrap();
        assert_eq!(remaining_events.len(), 1);
        assert_eq!(remaining_events[0].track_id, "tra_recent");
    }

    #[test]
    fn test_listening_events_deleted_on_user_delete() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record listening event
        let event = create_test_listening_event(user_id, "tra_001", 20241201);
        store.record_listening_event(event).unwrap();

        // Verify event exists
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);

        // Delete user (listening_events has ON DELETE CASCADE)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify events were deleted with user
        // Need to check directly in DB since user no longer exists
        {
            let conn = store.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM listening_events WHERE user_id = ?1",
                    params![user_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn test_listening_event_with_minimal_fields() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create event with only required fields, optional fields as None
        let event = ListeningEvent {
            id: None,
            user_id,
            track_id: "tra_minimal".to_string(),
            session_id: None,
            started_at: 1732982400,
            ended_at: None,
            duration_seconds: 100,
            track_duration_seconds: 200,
            completed: false,
            seek_count: 0,
            pause_count: 0,
            playback_context: None,
            client_type: None,
            date: 20241201,
        };

        let (id, created) = store.record_listening_event(event).unwrap();
        assert!(id > 0);
        // created=false because ended_at is None (event not finalized, won't count in metrics)
        assert!(!created);

        // Verify we can retrieve it
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].client_type.is_none());
        assert!(events[0].playback_context.is_none());
        assert!(events[0].ended_at.is_none());
    }

    #[test]
    fn test_listening_event_foreign_key_constraint() {
        let (store, _temp_dir) = create_tmp_store();

        // Try to insert event for non-existent user
        let event = create_test_listening_event(99999, "tra_001", 20241201);
        let result = store.record_listening_event(event);

        // Should fail due to foreign key constraint
        assert!(result.is_err());
    }

    #[test]
    fn test_get_user_listening_events_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Query with no events
        let events = store
            .get_user_listening_events(user_id, 20241201, 20241231, None, None)
            .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_user_listening_events_user_isolation() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // User 1 listens to track A
        let event1 = create_test_listening_event(user1_id, "tra_user1", 20241201);
        store.record_listening_event(event1).unwrap();

        // User 2 listens to track B
        let event2 = create_test_listening_event(user2_id, "tra_user2", 20241201);
        store.record_listening_event(event2).unwrap();

        // User 1 should only see their events
        let user1_events = store
            .get_user_listening_events(user1_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(user1_events.len(), 1);
        assert_eq!(user1_events[0].track_id, "tra_user1");

        // User 2 should only see their events
        let user2_events = store
            .get_user_listening_events(user2_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(user2_events.len(), 1);
        assert_eq!(user2_events[0].track_id, "tra_user2");
    }

    #[test]
    fn test_get_user_listening_summary_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get summary with no events
        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241231)
            .unwrap();

        assert_eq!(summary.user_id, Some(user_id));
        assert_eq!(summary.total_plays, 0);
        assert_eq!(summary.total_duration_seconds, 0);
        assert_eq!(summary.completed_plays, 0);
        assert_eq!(summary.unique_tracks, 0);
    }

    #[test]
    fn test_get_user_listening_history_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let history = store.get_user_listening_history(user_id, 10).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_get_user_listening_history_respects_limit() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 5 different tracks
        for i in 0..5 {
            let event = create_test_listening_event(user_id, &format!("tra_{:03}", i), 20241201);
            store.record_listening_event(event).unwrap();
        }

        // Request only 3
        let history = store.get_user_listening_history(user_id, 3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_get_track_listening_stats_nonexistent_track() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Record some events for a different track
        let event = create_test_listening_event(user_id, "tra_exists", 20241201);
        store.record_listening_event(event).unwrap();

        // Query stats for non-existent track
        let stats = store
            .get_track_listening_stats("tra_nonexistent", 20241201, 20241201)
            .unwrap();

        assert_eq!(stats.track_id, "tra_nonexistent");
        assert_eq!(stats.play_count, 0);
        assert_eq!(stats.total_duration_seconds, 0);
        assert_eq!(stats.completed_count, 0);
        assert_eq!(stats.unique_listeners, 0);
    }

    #[test]
    fn test_get_daily_listening_stats_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let _user_id = store.create_user("test_user").unwrap();

        // Query stats for date range with no events
        let daily_stats = store.get_daily_listening_stats(20241201, 20241231).unwrap();
        assert!(daily_stats.is_empty());
    }

    #[test]
    fn test_get_top_tracks_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let _user_id = store.create_user("test_user").unwrap();

        let top_tracks = store.get_top_tracks(20241201, 20241231, 10).unwrap();
        assert!(top_tracks.is_empty());
    }

    #[test]
    fn test_prune_listening_events_nothing_to_prune() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Get today's date
        let today = chrono::Utc::now();
        let today_date: u32 = today.format("%Y%m%d").to_string().parse().unwrap();

        // Record only recent events
        let recent_event = create_test_listening_event(user_id, "tra_recent", today_date);
        store.record_listening_event(recent_event).unwrap();

        // Prune events older than 30 days - nothing should be pruned
        let pruned = store.prune_listening_events(30).unwrap();
        assert_eq!(pruned, 0);

        // Verify event still exists
        let events = store
            .get_user_listening_events(user_id, today_date, today_date, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_session_id_protected_across_users() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        // User1 creates a session
        let mut event1 = create_test_listening_event(user1_id, "tra_001", 20241201);
        event1.session_id = Some("shared-session-id".to_string());

        let (id1, created1) = store.record_listening_event(event1).unwrap();
        assert!(created1);

        // User2 tries to use the same session_id - should be rejected
        // (session_id is globally unique and protected per user)
        let mut event2 = create_test_listening_event(user2_id, "tra_002", 20241201);
        event2.session_id = Some("shared-session-id".to_string());

        let (id2, created2) = store.record_listening_event(event2).unwrap();
        assert!(!created2); // Not created because session belongs to different user
        assert_eq!(id2, id1); // Returns the existing session's id

        // Verify user1's event is still intact
        let events = store
            .get_user_listening_events(user1_id, 20241201, 20241201, None, None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].track_id, "tra_001"); // Still user1's track
    }

    #[test]
    fn test_get_user_listening_events_ordering() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Insert events with different started_at times
        let mut event1 = create_test_listening_event(user_id, "tra_first", 20241201);
        event1.started_at = 1000;

        let mut event2 = create_test_listening_event(user_id, "tra_third", 20241201);
        event2.started_at = 3000;

        let mut event3 = create_test_listening_event(user_id, "tra_second", 20241201);
        event3.started_at = 2000;

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();
        store.record_listening_event(event3).unwrap();

        let events = store
            .get_user_listening_events(user_id, 20241201, 20241201, None, None)
            .unwrap();

        // Should be ordered by started_at descending (most recent first)
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].track_id, "tra_third");
        assert_eq!(events[1].track_id, "tra_second");
        assert_eq!(events[2].track_id, "tra_first");
    }

    #[test]
    fn test_completion_calculation_boundary() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Exactly 90% should be complete
        let mut event_90 = create_test_listening_event(user_id, "tra_90", 20241201);
        event_90.duration_seconds = 90;
        event_90.track_duration_seconds = 100;
        event_90.completed = true; // 90/100 = 0.90 = exactly 90%

        // 89% should not be complete
        let mut event_89 = create_test_listening_event(user_id, "tra_89", 20241201);
        event_89.duration_seconds = 89;
        event_89.track_duration_seconds = 100;
        event_89.completed = false; // 89/100 = 0.89 < 90%

        store.record_listening_event(event_90).unwrap();
        store.record_listening_event(event_89).unwrap();

        let summary = store
            .get_user_listening_summary(user_id, 20241201, 20241201)
            .unwrap();

        assert_eq!(summary.total_plays, 2);
        assert_eq!(summary.completed_plays, 1); // Only the 90% one
    }

    #[test]
    fn test_get_top_tracks_with_ties() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 3 tracks with same play count
        for track in &["tra_a", "tra_b", "tra_c"] {
            for _ in 0..5 {
                let event = create_test_listening_event(user_id, track, 20241201);
                store.record_listening_event(event).unwrap();
            }
        }

        let top_tracks = store.get_top_tracks(20241201, 20241201, 10).unwrap();

        assert_eq!(top_tracks.len(), 3);
        // All should have 5 plays
        for track in &top_tracks {
            assert_eq!(track.play_count, 5);
        }
    }

    #[test]
    fn test_daily_stats_multiple_days_gap() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Events on day 1 and day 5, nothing in between
        let event1 = create_test_listening_event(user_id, "tra_001", 20241201);
        let event2 = create_test_listening_event(user_id, "tra_002", 20241205);

        store.record_listening_event(event1).unwrap();
        store.record_listening_event(event2).unwrap();

        let daily_stats = store.get_daily_listening_stats(20241201, 20241205).unwrap();

        // Should only have 2 entries (days with actual events)
        assert_eq!(daily_stats.len(), 2);

        let dates: Vec<u32> = daily_stats.iter().map(|d| d.date).collect();
        assert!(dates.contains(&20241201));
        assert!(dates.contains(&20241205));
        // Days 2, 3, 4 should not be in results
        assert!(!dates.contains(&20241202));
        assert!(!dates.contains(&20241203));
        assert!(!dates.contains(&20241204));
    }

    // ==================== User Settings Tests ====================

    #[test]
    fn test_get_setting_returns_none_when_not_set() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_setting_returns_none_for_unknown_key() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let result = store.get_user_setting(user_id, "unknown_key").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_set_and_get_setting() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();

        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert_eq!(result, Some(UserSetting::NotifyWhatsNew(true)));
    }

    #[test]
    fn test_set_setting_overwrites_existing() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(false))
            .unwrap();
        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();

        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert_eq!(result, Some(UserSetting::NotifyWhatsNew(true)));
    }

    #[test]
    fn test_get_all_user_settings_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let settings = store.get_all_user_settings(user_id).unwrap();
        assert!(settings.is_empty());
    }

    #[test]
    fn test_get_all_user_settings_returns_known_settings() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();

        let settings = store.get_all_user_settings(user_id).unwrap();
        assert_eq!(settings.len(), 1);
        assert!(settings.contains(&UserSetting::NotifyWhatsNew(true)));
    }

    #[test]
    fn test_get_all_user_settings_skips_unknown_keys() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Set a known setting
        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();

        // Manually insert an unknown setting directly into the database
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
                 VALUES (?1, ?2, ?3, 0)",
                params![user_id, "unknown_future_setting", "some_value"],
            )
            .unwrap();
        }

        // get_all_user_settings should skip the unknown key
        let settings = store.get_all_user_settings(user_id).unwrap();
        assert_eq!(settings.len(), 1);
        assert!(settings.contains(&UserSetting::NotifyWhatsNew(true)));
    }

    #[test]
    fn test_settings_are_user_specific() {
        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        store
            .set_user_setting(user1_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();
        store
            .set_user_setting(user2_id, UserSetting::NotifyWhatsNew(false))
            .unwrap();

        let user1_value = store.get_user_setting(user1_id, "notify_whatsnew").unwrap();
        let user2_value = store.get_user_setting(user2_id, "notify_whatsnew").unwrap();

        assert_eq!(user1_value, Some(UserSetting::NotifyWhatsNew(true)));
        assert_eq!(user2_value, Some(UserSetting::NotifyWhatsNew(false)));
    }

    #[test]
    fn test_settings_deleted_with_user() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();

        // Delete the user via direct SQL (CASCADE should delete settings)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Verify settings are gone by checking the table directly
        {
            let conn = store.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM user_settings WHERE user_id = ?1",
                    params![user_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn test_notify_whatsnew_setting_lifecycle() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Default should be None (not set)
        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert!(result.is_none());

        // Set to true
        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(true))
            .unwrap();
        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert_eq!(result, Some(UserSetting::NotifyWhatsNew(true)));

        // Set to false
        store
            .set_user_setting(user_id, UserSetting::NotifyWhatsNew(false))
            .unwrap();
        let result = store.get_user_setting(user_id, "notify_whatsnew").unwrap();
        assert_eq!(result, Some(UserSetting::NotifyWhatsNew(false)));
    }

    // ========================================================================
    // Sync Event Log Tests
    // ========================================================================

    use crate::user::sync_events::{StoredEvent, UserEvent};
    use crate::user::user_models::LikedContentType;

    #[test]
    fn test_append_event_returns_sequence() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_123".to_string(),
        };

        let stored1 = store.append_event(user_id, &event).unwrap();
        let stored2 = store.append_event(user_id, &event).unwrap();

        assert!(stored1.seq > 0);
        assert!(stored2.seq > stored1.seq);
    }

    #[test]
    fn test_get_events_since_returns_events_in_order() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event1 = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };
        let event2 = UserEvent::ContentLiked {
            content_type: LikedContentType::Track,
            content_id: "track_1".to_string(),
        };
        let event3 = UserEvent::ContentUnliked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };

        let stored1 = store.append_event(user_id, &event1).unwrap();
        let stored2 = store.append_event(user_id, &event2).unwrap();
        let stored3 = store.append_event(user_id, &event3).unwrap();

        // Get all events (since 0)
        let events = store.get_events_since(user_id, 0).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, stored1.seq);
        assert_eq!(events[1].seq, stored2.seq);
        assert_eq!(events[2].seq, stored3.seq);

        // Get events since stored1.seq
        let events = store.get_events_since(user_id, stored1.seq).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq, stored2.seq);
        assert_eq!(events[1].seq, stored3.seq);

        // Get events since stored3.seq (should be empty)
        let events = store.get_events_since(user_id, stored3.seq).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_events_since_isolates_users() {
        let (store, _temp_dir) = create_tmp_store();
        let user1 = store.create_user("user1").unwrap();
        let user2 = store.create_user("user2").unwrap();

        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };

        store.append_event(user1, &event).unwrap();
        store.append_event(user1, &event).unwrap();
        store.append_event(user2, &event).unwrap();

        let events1 = store.get_events_since(user1, 0).unwrap();
        let events2 = store.get_events_since(user2, 0).unwrap();

        assert_eq!(events1.len(), 2);
        assert_eq!(events2.len(), 1);
    }

    #[test]
    fn test_get_current_seq_returns_zero_for_no_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let seq = store.get_current_seq(user_id).unwrap();
        assert_eq!(seq, 0);
    }

    #[test]
    fn test_get_current_seq_returns_latest_seq() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };

        let stored1 = store.append_event(user_id, &event).unwrap();
        assert_eq!(store.get_current_seq(user_id).unwrap(), stored1.seq);

        let stored2 = store.append_event(user_id, &event).unwrap();
        assert_eq!(store.get_current_seq(user_id).unwrap(), stored2.seq);
    }

    #[test]
    fn test_get_min_seq_returns_none_for_no_events() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let seq = store.get_min_seq(user_id).unwrap();
        assert!(seq.is_none());
    }

    #[test]
    fn deleting_missing_playlist_does_not_append_sync_event() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let error = store
            .delete_playlist_with_event("missing-playlist", user_id, Some("delete-missing"))
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<super::UserServiceError>(),
            Some(super::UserServiceError::NotFound(_))
        ));
        assert_eq!(store.get_current_seq(user_id).unwrap(), 0);
        assert!(store.get_events_since(user_id, 0).unwrap().is_empty());
    }

    #[test]
    fn test_get_min_seq_returns_first_seq() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };

        let stored1 = store.append_event(user_id, &event).unwrap();
        let _ = store.append_event(user_id, &event).unwrap();
        let _ = store.append_event(user_id, &event).unwrap();

        let min_seq = store.get_min_seq(user_id).unwrap();
        assert_eq!(min_seq, Some(stored1.seq));
    }

    #[test]
    fn test_prune_events_older_than() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_1".to_string(),
        };

        // Insert events
        store.append_event(user_id, &event).unwrap();
        store.append_event(user_id, &event).unwrap();
        store.append_event(user_id, &event).unwrap();

        // Get the current timestamp (events were just created)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Pruning with future timestamp should delete all events
        let deleted = store.prune_events_older_than(current_time + 10).unwrap();
        assert_eq!(deleted, 3);

        // Verify no events remain
        let events = store.get_events_since(user_id, 0).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Test various event types
        let events = vec![
            UserEvent::ContentLiked {
                content_type: LikedContentType::Artist,
                content_id: "artist_123".to_string(),
            },
            UserEvent::ContentUnliked {
                content_type: LikedContentType::Track,
                content_id: "track_456".to_string(),
            },
            UserEvent::SettingChanged {
                setting: UserSetting::NotifyWhatsNew(true),
            },
            UserEvent::PlaylistCreated {
                playlist_id: "pl_abc".to_string(),
                name: "My Playlist".to_string(),
            },
            UserEvent::PlaylistRenamed {
                playlist_id: "pl_abc".to_string(),
                name: "Renamed Playlist".to_string(),
            },
            UserEvent::PlaylistDeleted {
                playlist_id: "pl_abc".to_string(),
            },
            UserEvent::PlaylistTracksUpdated {
                playlist_id: "pl_abc".to_string(),
                track_ids: vec!["t1".to_string(), "t2".to_string()],
            },
            UserEvent::PermissionGranted {
                permission: Permission::EditCatalog,
            },
            UserEvent::PermissionRevoked {
                permission: Permission::RequestContent,
            },
            UserEvent::PermissionsReset {
                permissions: vec![Permission::AccessCatalog, Permission::LikeContent],
            },
        ];

        // Append all events
        for event in &events {
            store.append_event(user_id, event).unwrap();
        }

        // Retrieve and verify
        let stored_events = store.get_events_since(user_id, 0).unwrap();
        assert_eq!(stored_events.len(), events.len());

        for (original, stored) in events.iter().zip(stored_events.iter()) {
            assert_eq!(&stored.event, original);
        }
    }

    // ==================== DeviceStore Tests ====================

    #[test]
    fn test_register_new_device() {
        let (store, _temp_dir) = create_tmp_store();

        let reg = DeviceRegistration {
            device_uuid: "test-uuid-12345678".to_string(),
            device_type: DeviceType::Android,
            device_name: Some("Test Phone".to_string()),
            os_info: Some("Android 14".to_string()),
        };

        let device_id = store.register_or_update_device(&reg).unwrap();
        assert!(device_id > 0);

        let device = store.get_device(device_id).unwrap().unwrap();
        assert_eq!(device.device_uuid, "test-uuid-12345678");
        assert_eq!(device.device_type, DeviceType::Android);
        assert_eq!(device.device_name, Some("Test Phone".to_string()));
        assert_eq!(device.os_info, Some("Android 14".to_string()));
        assert!(device.user_id.is_none()); // Not associated with any user yet
    }

    #[test]
    fn test_register_existing_device_updates() {
        let (store, _temp_dir) = create_tmp_store();

        let reg1 = DeviceRegistration {
            device_uuid: "test-uuid-12345678".to_string(),
            device_type: DeviceType::Android,
            device_name: Some("Old Name".to_string()),
            os_info: None,
        };
        let id1 = store.register_or_update_device(&reg1).unwrap();

        // Register same device with updated info
        let reg2 = DeviceRegistration {
            device_uuid: "test-uuid-12345678".to_string(),
            device_type: DeviceType::Android,
            device_name: Some("New Name".to_string()),
            os_info: Some("Updated OS".to_string()),
        };
        let id2 = store.register_or_update_device(&reg2).unwrap();

        // Same device ID should be returned
        assert_eq!(id1, id2);

        // Verify info was updated
        let device = store.get_device(id1).unwrap().unwrap();
        assert_eq!(device.device_name, Some("New Name".to_string()));
        assert_eq!(device.os_info, Some("Updated OS".to_string()));
    }

    #[test]
    fn test_get_device_by_uuid() {
        let (store, _temp_dir) = create_tmp_store();

        let reg = DeviceRegistration {
            device_uuid: "unique-device-uuid".to_string(),
            device_type: DeviceType::Web,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        // Get by UUID
        let device = store
            .get_device_by_uuid("unique-device-uuid")
            .unwrap()
            .unwrap();
        assert_eq!(device.id, device_id);
        assert_eq!(device.device_type, DeviceType::Web);

        // Non-existent UUID returns None
        let not_found = store.get_device_by_uuid("non-existent-uuid").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_device_not_found() {
        let (store, _temp_dir) = create_tmp_store();

        let device = store.get_device(9999).unwrap();
        assert!(device.is_none());
    }

    #[test]
    fn test_associate_device_with_user() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let reg = DeviceRegistration {
            device_uuid: "assoc-test-uuid".to_string(),
            device_type: DeviceType::Web,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        // Initially no user
        let device = store.get_device(device_id).unwrap().unwrap();
        assert!(device.user_id.is_none());

        // Associate with user
        store
            .associate_device_with_user(device_id, user_id)
            .unwrap();

        let device = store.get_device(device_id).unwrap().unwrap();
        assert_eq!(device.user_id, Some(user_id));
    }

    #[test]
    fn test_get_user_devices() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Register and associate multiple devices
        for i in 0..3 {
            let reg = DeviceRegistration {
                device_uuid: format!("uuid-device-{}", i),
                device_type: DeviceType::Android,
                device_name: Some(format!("Device {}", i)),
                os_info: None,
            };
            let device_id = store.register_or_update_device(&reg).unwrap();
            store
                .associate_device_with_user(device_id, user_id)
                .unwrap();
        }

        let devices = store.get_user_devices(user_id).unwrap();
        assert_eq!(devices.len(), 3);

        // All devices should belong to the user
        for device in &devices {
            assert_eq!(device.user_id, Some(user_id));
        }
    }

    #[test]
    fn test_get_user_devices_empty() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // User with no devices
        let devices = store.get_user_devices(user_id).unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_touch_device_updates_last_seen() {
        let (store, _temp_dir) = create_tmp_store();

        let reg = DeviceRegistration {
            device_uuid: "touch-test-uuid".to_string(),
            device_type: DeviceType::Ios,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        let device_before = store.get_device(device_id).unwrap().unwrap();
        let last_seen_before = device_before.last_seen;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        store.touch_device(device_id).unwrap();

        let device_after = store.get_device(device_id).unwrap().unwrap();
        assert!(device_after.last_seen >= last_seen_before);
    }

    #[test]
    fn test_enforce_user_device_limit() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Register 5 devices with increasing last_seen times
        for i in 0..5 {
            let reg = DeviceRegistration {
                device_uuid: format!("limit-test-{}", i),
                device_type: DeviceType::Android,
                device_name: Some(format!("Device {}", i)),
                os_info: None,
            };
            let device_id = store.register_or_update_device(&reg).unwrap();
            store
                .associate_device_with_user(device_id, user_id)
                .unwrap();
            // Small delay to ensure different last_seen timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Verify we have 5 devices
        assert_eq!(store.get_user_devices(user_id).unwrap().len(), 5);

        // Enforce limit of 3
        let deleted = store.enforce_user_device_limit(user_id, 3).unwrap();
        assert_eq!(deleted, 2);

        let remaining = store.get_user_devices(user_id).unwrap();
        assert_eq!(remaining.len(), 3);

        // The oldest devices (0 and 1) should be deleted, keeping 2, 3, 4
        let uuids: Vec<&str> = remaining.iter().map(|d| d.device_uuid.as_str()).collect();
        assert!(!uuids.contains(&"limit-test-0"));
        assert!(!uuids.contains(&"limit-test-1"));
        assert!(uuids.contains(&"limit-test-2"));
        assert!(uuids.contains(&"limit-test-3"));
        assert!(uuids.contains(&"limit-test-4"));
    }

    #[test]
    fn test_enforce_user_device_limit_no_deletion_needed() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Register 2 devices
        for i in 0..2 {
            let reg = DeviceRegistration {
                device_uuid: format!("no-limit-{}", i),
                device_type: DeviceType::Web,
                device_name: None,
                os_info: None,
            };
            let device_id = store.register_or_update_device(&reg).unwrap();
            store
                .associate_device_with_user(device_id, user_id)
                .unwrap();
        }

        // Enforce limit of 5 (no deletion needed)
        let deleted = store.enforce_user_device_limit(user_id, 5).unwrap();
        assert_eq!(deleted, 0);

        assert_eq!(store.get_user_devices(user_id).unwrap().len(), 2);
    }

    #[test]
    fn test_prune_orphaned_devices() {
        let (store, _temp_dir) = create_tmp_store();

        // Create an orphaned device (no user_id) with old timestamp
        let reg = DeviceRegistration {
            device_uuid: "orphan-uuid-test".to_string(),
            device_type: DeviceType::Web,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        // Manually set last_seen to 10 days ago
        {
            let conn = store.conn.lock().unwrap();
            let ten_days_ago = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - (10 * 24 * 60 * 60);
            conn.execute(
                "UPDATE device SET last_seen = ?1 WHERE id = ?2",
                params![ten_days_ago, device_id],
            )
            .unwrap();
        }

        // Verify device exists
        assert!(store.get_device(device_id).unwrap().is_some());

        // Prune devices inactive for more than 7 days
        let deleted = store.prune_orphaned_devices(7).unwrap();
        assert_eq!(deleted, 1);

        // Device should be gone
        assert!(store.get_device(device_id).unwrap().is_none());
    }

    #[test]
    fn test_prune_orphaned_devices_does_not_delete_associated() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a device associated with user with old timestamp
        let reg = DeviceRegistration {
            device_uuid: "associated-uuid".to_string(),
            device_type: DeviceType::Android,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();
        store
            .associate_device_with_user(device_id, user_id)
            .unwrap();

        // Manually set last_seen to 10 days ago
        {
            let conn = store.conn.lock().unwrap();
            let ten_days_ago = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - (10 * 24 * 60 * 60);
            conn.execute(
                "UPDATE device SET last_seen = ?1 WHERE id = ?2",
                params![ten_days_ago, device_id],
            )
            .unwrap();
        }

        // Prune orphaned devices - should not delete associated devices
        let deleted = store.prune_orphaned_devices(7).unwrap();
        assert_eq!(deleted, 0);

        // Device should still exist
        assert!(store.get_device(device_id).unwrap().is_some());
    }

    #[test]
    fn test_prune_orphaned_devices_does_not_delete_recent() {
        let (store, _temp_dir) = create_tmp_store();

        // Create an orphaned device (no user_id) with recent timestamp
        let reg = DeviceRegistration {
            device_uuid: "recent-orphan-uuid".to_string(),
            device_type: DeviceType::Web,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        // Prune devices inactive for more than 7 days - should not delete recent device
        let deleted = store.prune_orphaned_devices(7).unwrap();
        assert_eq!(deleted, 0);

        // Device should still exist
        assert!(store.get_device(device_id).unwrap().is_some());
    }

    #[test]
    fn test_device_user_id_set_null_on_user_delete() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a device and associate with user
        let reg = DeviceRegistration {
            device_uuid: "cascade-test-uuid".to_string(),
            device_type: DeviceType::Android,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();
        store
            .associate_device_with_user(device_id, user_id)
            .unwrap();

        // Verify association
        let device = store.get_device(device_id).unwrap().unwrap();
        assert_eq!(device.user_id, Some(user_id));

        // Delete user
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM user WHERE id = ?1", params![user_id])
                .unwrap();
        }

        // Device should still exist but user_id should be NULL (ON DELETE SET NULL)
        let device = store.get_device(device_id).unwrap().unwrap();
        assert!(device.user_id.is_none());
    }

    // ==================== AuthToken with Device Tests ====================

    #[test]
    fn test_auth_token_with_device_id() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a device
        let reg = DeviceRegistration {
            device_uuid: "token-test-uuid".to_string(),
            device_type: DeviceType::Web,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();

        // Create token with device_id
        let token = AuthToken {
            user_id,
            device_id: Some(device_id),
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };

        store.add_user_auth_token(token.clone()).unwrap();

        // Retrieve and verify
        let retrieved = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert_eq!(retrieved.device_id, Some(device_id));
        assert_eq!(retrieved.user_id, user_id);
    }

    #[test]
    fn test_auth_token_without_device_id() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create token without device_id (backward compatibility)
        let token = AuthToken {
            user_id,
            device_id: None,
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };

        store.add_user_auth_token(token.clone()).unwrap();

        // Retrieve and verify
        let retrieved = store.get_user_auth_token(&token.value).unwrap().unwrap();
        assert!(retrieved.device_id.is_none());
    }

    // =========================================================================
    // Notification Store Tests
    // =========================================================================

    #[test]
    fn test_create_notification() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let notification = store
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Album Ready".to_string(),
                Some("Your album is ready".to_string()),
                serde_json::json!({ "album_id": "album-123" }),
            )
            .unwrap();

        assert!(notification.id.starts_with("notif_"));
        assert_eq!(
            notification.notification_type,
            NotificationType::DownloadCompleted
        );
        assert_eq!(notification.title, "Album Ready");
        assert_eq!(notification.body, Some("Your album is ready".to_string()));
        assert!(notification.read_at.is_none());
    }

    #[test]
    fn test_get_user_notifications() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create multiple notifications
        for i in 0..3 {
            store
                .create_notification(
                    user_id,
                    NotificationType::DownloadCompleted,
                    format!("Notification {}", i),
                    None,
                    serde_json::json!({ "index": i }),
                )
                .unwrap();
        }

        let notifications = store.get_user_notifications(user_id).unwrap();
        assert_eq!(notifications.len(), 3);
        // Should be ordered by created_at DESC (newest first)
        assert!(notifications[0].created_at >= notifications[1].created_at);
        assert!(notifications[1].created_at >= notifications[2].created_at);
    }

    #[test]
    fn test_notification_100_limit_enforcement() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 105 notifications
        for i in 0..105 {
            store
                .create_notification(
                    user_id,
                    NotificationType::DownloadCompleted,
                    format!("Notification {}", i),
                    None,
                    serde_json::json!({ "index": i }),
                )
                .unwrap();
        }

        // Should only have 100 notifications (limit enforced)
        let notifications = store.get_user_notifications(user_id).unwrap();
        assert_eq!(notifications.len(), 100);

        // The latest notification (104) should definitely be present
        let titles: Vec<&str> = notifications.iter().map(|n| n.title.as_str()).collect();
        assert!(titles.contains(&"Notification 104"));
    }

    #[test]
    fn test_mark_notification_read() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let notification = store
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                None,
                serde_json::json!({}),
            )
            .unwrap();

        assert!(notification.read_at.is_none());

        let updated = store
            .mark_notification_read(&notification.id, user_id)
            .unwrap()
            .unwrap();

        assert!(updated.read_at.is_some());
    }

    #[test]
    fn test_mark_notification_read_idempotent() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let notification = store
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                None,
                serde_json::json!({}),
            )
            .unwrap();

        let first_mark = store
            .mark_notification_read(&notification.id, user_id)
            .unwrap()
            .unwrap();
        let first_read_at = first_mark.read_at.unwrap();

        // Marking again should not change the read_at
        let second_mark = store
            .mark_notification_read(&notification.id, user_id)
            .unwrap()
            .unwrap();

        assert_eq!(second_mark.read_at.unwrap(), first_read_at);
    }

    #[test]
    fn test_mark_notification_read_wrong_user() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        let notification = store
            .create_notification(
                user1_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                None,
                serde_json::json!({}),
            )
            .unwrap();

        // User2 trying to mark user1's notification as read
        let result = store
            .mark_notification_read(&notification.id, user2_id)
            .unwrap();

        // Should return None since notification doesn't belong to user2
        assert!(result.is_none());
    }

    #[test]
    fn test_get_unread_count() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create 5 notifications
        let mut notification_ids = Vec::new();
        for i in 0..5 {
            let notification = store
                .create_notification(
                    user_id,
                    NotificationType::DownloadCompleted,
                    format!("Notification {}", i),
                    None,
                    serde_json::json!({}),
                )
                .unwrap();
            notification_ids.push(notification.id);
        }

        assert_eq!(store.get_unread_count(user_id).unwrap(), 5);

        // Mark 2 as read
        store
            .mark_notification_read(&notification_ids[0], user_id)
            .unwrap();
        store
            .mark_notification_read(&notification_ids[2], user_id)
            .unwrap();

        assert_eq!(store.get_unread_count(user_id).unwrap(), 3);
    }

    #[test]
    fn test_get_notification() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        let created = store
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                Some("Body".to_string()),
                serde_json::json!({ "key": "value" }),
            )
            .unwrap();

        let fetched = store
            .get_notification(&created.id, user_id)
            .unwrap()
            .unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.title, "Test");
        assert_eq!(fetched.body, Some("Body".to_string()));
        assert_eq!(fetched.data, serde_json::json!({ "key": "value" }));
    }

    #[test]
    fn test_get_notification_wrong_user() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();

        let notification = store
            .create_notification(
                user1_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                None,
                serde_json::json!({}),
            )
            .unwrap();

        // User2 trying to get user1's notification
        let result = store.get_notification(&notification.id, user2_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_notifications_cascade_delete_on_user_delete() {
        use crate::notifications::{NotificationStore, NotificationType};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("test_user").unwrap();

        // Create a notification
        let notification = store
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Test".to_string(),
                None,
                serde_json::json!({}),
            )
            .unwrap();

        // Verify notification exists
        assert!(store
            .get_notification(&notification.id, user_id)
            .unwrap()
            .is_some());

        // Delete the user
        store.delete_user(user_id).unwrap();

        // Try to get notifications for the deleted user - should be empty
        // (foreign key cascade delete should have removed notifications)
        let notifications = store.get_user_notifications(user_id).unwrap();
        assert!(notifications.is_empty());
    }

    #[test]
    fn atomic_like_rolls_back_when_event_insert_fails() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("atomic_like_user").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_user_event BEFORE INSERT ON user_events
                 BEGIN SELECT RAISE(FAIL, 'injected event failure'); END;",
            )
            .unwrap();

        let result = store.set_liked_content_with_event(
            user_id,
            "track-1",
            LikedContentType::Track,
            true,
            Some("failed-like"),
        );

        assert!(result.is_err());
        assert_eq!(
            store
                .get_user_liked_content(user_id, LikedContentType::Track)
                .unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(store.get_current_seq(user_id).unwrap(), 0);
    }

    #[test]
    fn atomic_role_change_rolls_back_when_event_insert_fails() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("atomic_role_user").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_user_event BEFORE INSERT ON user_events
                 BEGIN SELECT RAISE(FAIL, 'injected event failure'); END;",
            )
            .unwrap();

        let result = store.set_user_role_with_event(user_id, UserRole::Admin, true);

        assert!(result.is_err());
        assert!(store.get_user_roles(user_id).unwrap().is_empty());
        assert_eq!(store.get_current_seq(user_id).unwrap(), 0);
    }

    #[test]
    fn atomic_extra_permission_changes_roll_back_when_event_insert_fails() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("atomic_permission_user").unwrap();
        let grant = PermissionGrant::Extra {
            start_time: SystemTime::now(),
            end_time: None,
            permission: Permission::ServerAdmin,
            countdown: None,
        };
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_user_event BEFORE INSERT ON user_events
                 BEGIN SELECT RAISE(FAIL, 'injected event failure'); END;",
            )
            .unwrap();

        assert!(store
            .add_extra_permission_with_event(user_id, grant)
            .is_err());
        assert_eq!(
            store.resolve_user_permissions(user_id).unwrap(),
            Vec::<Permission>::new()
        );

        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_user_event;")
            .unwrap();
        let permission_id = store
            .add_user_extra_permission(
                user_id,
                PermissionGrant::Extra {
                    start_time: SystemTime::now(),
                    end_time: None,
                    permission: Permission::ServerAdmin,
                    countdown: None,
                },
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_user_event BEFORE INSERT ON user_events
                 BEGIN SELECT RAISE(FAIL, 'injected event failure'); END;",
            )
            .unwrap();

        assert!(store
            .remove_extra_permission_with_event(permission_id)
            .is_err());
        assert!(store
            .resolve_user_permissions(user_id)
            .unwrap()
            .contains(&Permission::ServerAdmin));
        assert_eq!(store.get_current_seq(user_id).unwrap(), 0);
    }

    #[test]
    fn settings_batch_rolls_back_all_values_and_events_on_failure() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("atomic_settings_user").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_second_user_event BEFORE INSERT ON user_events
                 WHEN NEW.operation_index = 1
                 BEGIN SELECT RAISE(FAIL, 'injected second event failure'); END;",
            )
            .unwrap();

        let result = store.set_settings_with_events(
            user_id,
            vec![
                UserSetting::NotifyWhatsNew(true),
                UserSetting::SmartContinuationEnabled(true),
            ],
            Some("failed-settings"),
        );

        assert!(result.is_err());
        assert!(store.get_all_user_settings(user_id).unwrap().is_empty());
        assert_eq!(store.get_current_seq(user_id).unwrap(), 0);
    }

    #[test]
    fn mutation_operation_ids_are_idempotent() {
        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("idempotent_user").unwrap();
        let first = store
            .set_liked_content_with_event(
                user_id,
                "album-1",
                LikedContentType::Album,
                true,
                Some("like-operation-1"),
            )
            .unwrap();
        let replay = store
            .set_liked_content_with_event(
                user_id,
                "album-1",
                LikedContentType::Album,
                true,
                Some("like-operation-1"),
            )
            .unwrap();

        assert_eq!(replay, first);
        assert_eq!(store.get_current_seq(user_id).unwrap(), first.seq);
        assert_eq!(
            store
                .get_user_liked_content(user_id, LikedContentType::Album)
                .unwrap(),
            vec!["album-1".to_string()]
        );
    }

    #[test]
    fn sync_snapshot_sequence_and_state_are_consistent_during_writes() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("snapshot_user").unwrap();
        let store = Arc::new(store);
        let finished = Arc::new(AtomicBool::new(false));
        let writer_store = store.clone();
        let writer_finished = finished.clone();
        let writer = std::thread::spawn(move || {
            for index in 1..=200 {
                writer_store
                    .set_liked_content_with_event(
                        user_id,
                        "track-1",
                        LikedContentType::Track,
                        index % 2 == 1,
                        Some(&format!("snapshot-operation-{index}")),
                    )
                    .unwrap();
            }
            writer_finished.store(true, Ordering::Release);
        });

        while !finished.load(Ordering::Acquire) {
            let snapshot = store.get_sync_snapshot(user_id).unwrap();
            let liked = snapshot.liked_tracks.iter().any(|id| id == "track-1");
            assert_eq!(liked, snapshot.seq % 2 == 1);
        }
        writer.join().unwrap();
        let snapshot = store.get_sync_snapshot(user_id).unwrap();
        assert_eq!(snapshot.seq, 200);
        assert!(snapshot.liked_tracks.is_empty());
    }

    #[test]
    fn cloned_store_serializes_concurrent_writes_without_losing_updates() {
        const WRITERS: usize = 32;

        let (store, _temp_dir) = create_tmp_store();
        let user_id = store.create_user("concurrent_store_user").unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(WRITERS));

        let writers: Vec<_> = (0..WRITERS)
            .map(|index| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .set_user_liked_content(
                            user_id,
                            &format!("track-{index}"),
                            LikedContentType::Track,
                            true,
                        )
                        .unwrap();
                })
            })
            .collect();

        for writer in writers {
            writer.join().unwrap();
        }

        let mut liked = store
            .get_user_liked_content(user_id, LikedContentType::Track)
            .unwrap();
        liked.sort();
        assert_eq!(liked.len(), WRITERS);
        for index in 0..WRITERS {
            assert!(liked.contains(&format!("track-{index}")));
        }
    }
}
