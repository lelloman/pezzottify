#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::schema::CATALOG_VERSIONED_SCHEMAS;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn create_test_store() -> (SqliteCatalogStore, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = SqliteCatalogStore::new(
            temp_dir.path().join("test.db"),
            temp_dir.path(),
            2,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        (store, temp_dir)
    }

    #[test]
    fn float_vector_blob_round_trip_and_length_validation() {
        let vector = [0.0, -1.25, f32::MIN_POSITIVE, f32::MAX, f32::NAN];
        let encoded = SqliteCatalogStore::encode_f32_vector(&vector);
        let decoded = SqliteCatalogStore::decode_f32_vector(&encoded).unwrap();

        assert_eq!(decoded.len(), vector.len());
        for (actual, expected) in decoded.iter().zip(vector) {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }

        let error = SqliteCatalogStore::decode_f32_vector(&encoded[..encoded.len() - 1])
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("invalid float32 vector blob length"));
    }

    #[test]
    fn catalog_mutations_return_typed_expected_errors() {
        let (store, _temp_dir) = create_test_store();
        let album = Album {
            id: "album".to_owned(),
            name: "Album".to_owned(),
            album_type: AlbumType::Album,
            label: None,
            release_date: Some("2026".to_owned()),
            release_date_precision: Some("year".to_owned()),
            external_id_upc: None,
            popularity: 0,
            album_availability: AlbumAvailability::Missing,
        };

        let error = store
            .create_album(&album, &["missing-artist".to_owned()])
            .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<CatalogMutationError>(),
                Some(CatalogMutationError::InvalidReference {
                    entity: "Artist",
                    id,
                }) if id == "missing-artist"
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn test_catalog_migrations_do_not_backfill_queue_or_cardinality_stats() {
        let mut conn = Connection::open_in_memory().unwrap();
        CATALOG_VERSIONED_SCHEMAS[7].create(&conn).unwrap();
        conn.pragma_update(None, "user_version", BASE_DB_VERSION + 7)
            .unwrap();
        for index in 0..100 {
            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity)
                 VALUES (?1, ?1, 0, 0)",
                params![format!("artist-{index}")],
            )
            .unwrap();
        }

        migrate_if_needed(&mut conn).unwrap();
        let queued: i64 = conn
            .query_row("SELECT COUNT(*) FROM artist_enrichment_queue", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(queued, 0, "migration must not backfill the catalog");

        let stats_valid: i64 = conn
            .query_row(
                "SELECT is_valid FROM catalog_stats WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stats_valid, 0, "migration must not count the catalog");

        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity)
             VALUES ('after-migration', 'After Migration', 0, 0)",
            [],
        )
        .unwrap();
        let queued_after_insert: i64 = conn
            .query_row("SELECT COUNT(*) FROM artist_enrichment_queue", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(queued_after_insert, 1);
    }

    #[test]
    fn test_catalog_cardinality_stats_rebuild_and_incremental_updates() {
        let (store, _temp_dir) = create_test_store();
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "UPDATE catalog_stats
                 SET artists_count = NULL, albums_count = NULL, tracks_count = NULL,
                     is_valid = 0",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity)
                 VALUES ('artist', 'Artist', 0, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO albums
                 (id, name, album_type, label, popularity, release_date, release_date_precision)
                 VALUES ('album', 'Album', 'album', '', 0, '2026', 'year')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks
                 (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
                 VALUES ('track', 'Track', (SELECT rowid FROM albums WHERE id = 'album'),
                         1, 0, 1, 1000, 0)",
                [],
            )
            .unwrap();
        }

        assert!(store.get_catalog_cardinality_stats().unwrap().is_none());
        let rebuilt = store
            .rebuild_catalog_cardinality_stats(Arc::new(|| false))
            .unwrap();
        assert_eq!((rebuilt.artists, rebuilt.albums, rebuilt.tracks), (1, 1, 1));
        assert_eq!(store.get_artists_count(), 1);
        assert_eq!(store.get_albums_count(), 1);
        assert_eq!(store.get_tracks_count(), 1);

        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity)
                 VALUES ('artist-2', 'Artist 2', 0, 0)",
                [],
            )
            .unwrap();
            conn.execute("DELETE FROM tracks WHERE id = 'track'", [])
                .unwrap();
        }
        let updated = store.get_catalog_cardinality_stats().unwrap().unwrap();
        assert_eq!((updated.artists, updated.albums, updated.tracks), (2, 1, 0));
    }

    #[test]
    fn test_catalog_cardinality_stats_rebuild_is_cancellable() {
        let (store, _temp_dir) = create_test_store();
        let before = store.get_catalog_cardinality_stats().unwrap().unwrap();
        let error = store
            .rebuild_catalog_cardinality_stats(Arc::new(|| true))
            .unwrap_err();
        assert_eq!(error.to_string(), "cancelled");
        assert_eq!(store.get_catalog_cardinality_stats().unwrap(), Some(before));
    }

    #[test]
    fn test_catalog_cardinality_index_scan_is_cancellable_while_running() {
        let (store, _temp_dir) = create_test_store();
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute_batch(
                "WITH RECURSIVE ids(value) AS (
                     SELECT 1 UNION ALL SELECT value + 1 FROM ids WHERE value < 5000
                 )
                 INSERT INTO artists (id, name, followers_total, popularity)
                 SELECT 'artist-' || value, 'Artist ' || value, 0, 0 FROM ids",
            )
            .unwrap();
        }

        let checks = Arc::new(AtomicUsize::new(0));
        let callback_checks = checks.clone();
        let error = store
            .count_table_rows_cancellable(
                "artists",
                "idx_artists_available",
                Arc::new(move || callback_checks.fetch_add(1, Ordering::SeqCst) > 0),
            )
            .unwrap_err();

        assert_eq!(error.to_string(), "cancelled");
        assert!(checks.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn test_get_artist_rowids_by_mbids_batches_matches_and_omits_missing() {
        let (store, _temp_dir) = create_test_store();
        {
            let conn = store.write_conn.lock().unwrap();
            for (id, mbid) in [
                ("artist-1", "mbid-1"),
                ("artist-2", "mbid-2"),
                ("artist-3", "mbid-3"),
            ] {
                conn.execute(
                    "INSERT INTO artists
                     (id, name, followers_total, popularity, artist_available, mbid, mbid_lookup_status)
                     VALUES (?1, ?1, 0, 0, 0, ?2, 1)",
                    params![id, mbid],
                )
                .unwrap();
            }
        }

        let requested = vec![
            "mbid-2".to_string(),
            "missing".to_string(),
            "mbid-1".to_string(),
        ];
        let resolved = store
            .get_artist_rowids_by_mbids(&requested)
            .unwrap()
            .into_iter()
            .collect::<HashMap<_, _>>();

        assert_eq!(resolved.len(), 2);
        assert!(resolved.contains_key("mbid-1"));
        assert!(resolved.contains_key("mbid-2"));
        assert!(!resolved.contains_key("missing"));
        assert!(store.get_artist_rowids_by_mbids(&[]).unwrap().is_empty());
    }

    #[test]
    fn test_artist_enrichment_queue_retries_without_starving_next_artist() {
        let (store, _temp_dir) = create_test_store();
        let (high_rowid, low_rowid) = {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists
                 (id, name, followers_total, popularity, artist_available)
                 VALUES ('high', 'High', 0, 90, 1), ('low', 'Low', 0, 10, 0)",
                [],
            )
            .unwrap();
            let high = conn
                .query_row("SELECT rowid FROM artists WHERE id = 'high'", [], |row| {
                    row.get(0)
                })
                .unwrap();
            let low = conn
                .query_row("SELECT rowid FROM artists WHERE id = 'low'", [], |row| {
                    row.get(0)
                })
                .unwrap();
            (high, low)
        };

        let first = store.get_artists_needing_mbid(1).unwrap();
        assert_eq!(first, vec![("high".to_string(), high_rowid)]);
        store
            .record_artist_mbid_failure(high_rowid, "temporary upstream failure")
            .unwrap();

        let second = store.get_artists_needing_mbid(1).unwrap();
        assert_eq!(second, vec![("low".to_string(), low_rowid)]);

        let conn = store.write_conn.lock().unwrap();
        let state: (String, i64, i64, String) = conn
            .query_row(
                "SELECT status, attempt_count, next_attempt_at, last_error
                 FROM artist_enrichment_queue
                 WHERE artist_rowid = ?1 AND phase = 'mbid'",
                params![high_rowid],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(state.0, "queued");
        assert_eq!(state.1, 1);
        assert!(state.2 > Utc::now().timestamp());
        assert_eq!(state.3, "temporary upstream failure");
    }

    #[test]
    fn test_artist_enrichment_queue_tracks_independent_phases_to_completion() {
        let (store, _temp_dir) = create_test_store();
        let artist_rowid = {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists
                 (id, name, followers_total, popularity, artist_available)
                 VALUES ('artist', 'Artist', 0, 50, 1)",
                [],
            )
            .unwrap();
            conn.last_insert_rowid()
        };

        assert_eq!(store.get_artists_needing_mbid(1).unwrap().len(), 1);
        store.set_artist_mbid("artist", "mbid-artist").unwrap();

        let related = store.get_artists_needing_related(1).unwrap();
        assert_eq!(
            related,
            vec![(
                "artist".to_string(),
                "mbid-artist".to_string(),
                artist_rowid
            )]
        );
        store.set_related_artists(artist_rowid, &[]).unwrap();

        let conn = store.write_conn.lock().unwrap();
        let states = conn
            .prepare(
                "SELECT phase, status FROM artist_enrichment_queue
                 WHERE artist_rowid = ?1 ORDER BY phase",
            )
            .unwrap()
            .query_map(params![artist_rowid], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            states,
            vec![
                ("mbid".to_string(), "completed".to_string()),
                ("related".to_string(), "completed".to_string()),
            ]
        );
    }

    #[test]
    fn test_artist_enrichment_queue_quarantines_after_bounded_attempts() {
        let (store, _temp_dir) = create_test_store();
        let artist_rowid = {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists
                 (id, name, followers_total, popularity, artist_available)
                 VALUES ('poison', 'Poison', 0, 99, 1)",
                [],
            )
            .unwrap();
            conn.last_insert_rowid()
        };

        for _ in 0..ENRICHMENT_MAX_ATTEMPTS {
            let claimed = store.get_artists_needing_mbid(1).unwrap();
            assert_eq!(claimed.len(), 1);
            store
                .record_artist_mbid_failure(artist_rowid, "still failing")
                .unwrap();
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "UPDATE artist_enrichment_queue SET next_attempt_at = 0
                 WHERE artist_rowid = ?1 AND status = 'queued'",
                params![artist_rowid],
            )
            .unwrap();
        }

        assert!(store.get_artists_needing_mbid(1).unwrap().is_empty());
        let conn = store.write_conn.lock().unwrap();
        let state: (String, i64, Option<i64>) = conn
            .query_row(
                "SELECT status, attempt_count, next_attempt_at
                 FROM artist_enrichment_queue WHERE artist_rowid = ?1",
                params![artist_rowid],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, ("permanent_failure".to_string(), 8, None));
    }

    fn sample_embedding(
        entity_id: &str,
        namespace: &str,
        vector: Vec<f32>,
    ) -> EntityEmbeddingUpsert {
        EntityEmbeddingUpsert {
            entity_type: "track".to_string(),
            entity_id: entity_id.to_string(),
            namespace: namespace.to_string(),
            vector,
            dtype: "float32".to_string(),
            metadata: serde_json::json!({
                "source": "unit-test",
                "normalized": false
            }),
            model: serde_json::json!({
                "name": "test-model",
                "version": "v1"
            }),
        }
    }

    fn seed_available_track(
        store: &SqliteCatalogStore,
        track_id: &str,
        audio_uri: Option<&str>,
        available: bool,
    ) {
        let conn = store.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO artists (id, name, followers_total, popularity, artist_available, mbid_lookup_status)
             VALUES ('artist1', 'Artist 1', 0, 0, 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
             VALUES ('album1', 'Album 1', 'album', '', 0, '2024', 'year', 'missing')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, audio_uri, track_available)
             VALUES (?1, ?2, (SELECT rowid FROM albums WHERE id='album1'), 1, 0, 1, 1000, 0, ?3, ?4)",
            params![
                track_id,
                format!("Track {track_id}"),
                audio_uri,
                if available { 1 } else { 0 }
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role)
             VALUES ((SELECT rowid FROM tracks WHERE id=?1), (SELECT rowid FROM artists WHERE id='artist1'), 0)",
            params![track_id],
        )
        .unwrap();
    }

    #[test]
    fn media_identifiers_require_normalized_relative_paths() {
        for valid in [
            "audio/ab/cd/track.ogg",
            "audio/café/音楽.ogg",
            "audio/．．/fullwidth-dots-are-literal.ogg",
            "audio/%2e%2e/literal.ogg",
        ] {
            assert_eq!(
                normalized_media_identifier(valid).unwrap(),
                PathBuf::from(valid),
                "valid identifier: {valid}"
            );
        }

        for invalid in [
            "",
            "/etc/passwd",
            "../outside.ogg",
            "audio/../outside.ogg",
            "./audio.ogg",
            "audio/./track.ogg",
            "audio//track.ogg",
            "audio/",
            "audio\\track.ogg",
            "C:\\Windows\\system.ini",
            "audio/\0track.ogg",
        ] {
            assert!(
                normalized_media_identifier(invalid).is_err(),
                "invalid identifier: {invalid:?}"
            );
        }
    }

    #[test]
    fn media_path_resolution_rejects_absolute_traversal_and_symlink_escape() {
        let root = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("audio/café")).unwrap();
        std::fs::write(root.path().join("audio/café/音楽.ogg"), b"inside").unwrap();
        std::fs::write(outside.path().join("secret.ogg"), b"outside").unwrap();

        let resolved = resolve_existing_media_path(root.path(), "audio/café/音楽.ogg").unwrap();
        assert!(resolved.starts_with(root.path().canonicalize().unwrap()));
        assert!(resolve_existing_media_path(root.path(), "../secret.ogg").is_err());
        assert!(resolve_existing_media_path(
            root.path(),
            outside.path().join("secret.ogg").to_str().unwrap()
        )
        .is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                outside.path().join("secret.ogg"),
                root.path().join("audio/escape.ogg"),
            )
            .unwrap();
            assert!(resolve_existing_media_path(root.path(), "audio/escape.ogg").is_err());
            assert!(open_media_file_beneath(root.path(), "audio/escape.ogg").is_err());

            std::os::unix::fs::symlink(outside.path(), root.path().join("linked-directory"))
                .unwrap();
            assert!(
                resolve_existing_media_path(root.path(), "linked-directory/secret.ogg").is_err()
            );
            assert!(open_media_file_beneath(root.path(), "linked-directory/secret.ogg").is_err());
        }
    }

    #[test]
    fn safe_media_open_reads_regular_file_beneath_root() {
        use std::io::Read;

        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("audio/subdir")).unwrap();
        std::fs::write(root.path().join("audio/subdir/track.ogg"), b"audio bytes").unwrap();

        let (mut file, path) =
            open_media_file_beneath(root.path(), "audio/subdir/track.ogg").unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"audio bytes");
        assert_eq!(path, root.path().join("audio/subdir/track.ogg"));
    }

    #[test]
    fn catalog_rejects_unsafe_or_missing_audio_uri_at_write_boundary() {
        let (store, temp_dir) = create_test_store();
        seed_available_track(&store, "track-safe-path", None, false);
        std::fs::create_dir_all(temp_dir.path().join("audio")).unwrap();
        std::fs::write(temp_dir.path().join("audio/track.ogg"), b"audio").unwrap();

        assert!(store
            .set_track_audio_uri("track-safe-path", "audio/track.ogg")
            .is_ok());
        assert!(store
            .set_track_audio_uri("track-safe-path", "../outside.ogg")
            .is_err());
        assert!(store
            .set_track_audio_uri("track-safe-path", "/etc/passwd")
            .is_err());
        assert!(store
            .set_track_audio_uri("track-safe-path", "audio/missing.ogg")
            .is_err());
    }

    #[test]
    fn imported_unsafe_audio_uri_is_never_exposed_as_a_file_path() {
        let (store, temp_dir) = create_test_store();
        let outside = tempfile::TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret.ogg"), b"secret").unwrap();
        let traversal = format!(
            "../{}/secret.ogg",
            outside.path().file_name().unwrap().to_string_lossy()
        );
        seed_available_track(&store, "imported-unsafe", Some(&traversal), true);

        assert!(store.get_track_audio_path("imported-unsafe").is_none());
        assert!(store.open_track_audio_file("imported-unsafe").is_err());

        // Ensure the test setup really points at an existing file outside the root.
        assert!(temp_dir.path().join(&traversal).exists());
    }

    #[tokio::test]
    async fn cloned_catalog_store_handles_concurrent_reads_consistently() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = SqliteCatalogStore::new(
            temp_dir.path().join("test.db"),
            temp_dir.path(),
            4,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                tokio::spawn({
                    let store = store.clone();
                    async move {
                        for _ in 0..100 {
                            assert_eq!(store.get_artists_count(), 0);
                            assert_eq!(store.get_albums_count(), 0);
                            assert_eq!(store.get_tracks_count(), 0);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[test]
    fn test_entity_embedding_upsert_get_list_and_delete() {
        let (store, _temp_dir) = create_test_store();
        let upsert = sample_embedding("track1", "musicfm.mean.v1", vec![1.0, 2.0, 2.0]);

        let stored = store.upsert_entity_embedding(&upsert).unwrap();
        assert_eq!(stored.entity_type, "track");
        assert_eq!(stored.entity_id, "track1");
        assert_eq!(stored.namespace, "musicfm.mean.v1");
        assert_eq!(stored.dim, 3);
        assert_eq!(stored.dtype, "float32");
        assert_eq!(stored.vector.as_ref().unwrap(), &vec![1.0, 2.0, 2.0]);
        assert!((stored.vector_norm - 3.0).abs() < 1e-9);
        assert_eq!(stored.metadata["source"], "unit-test");
        assert_eq!(stored.model["name"], "test-model");

        let without_vector = store
            .get_entity_embedding("track", "track1", "musicfm.mean.v1", false)
            .unwrap()
            .unwrap();
        assert!(without_vector.vector.is_none());

        let listed = store
            .list_entity_embeddings("track", "track1", false)
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].namespace, "musicfm.mean.v1");
        assert!(listed[0].vector.is_none());

        assert!(store
            .delete_entity_embedding("track", "track1", "musicfm.mean.v1")
            .unwrap());
        assert!(store
            .get_entity_embedding("track", "track1", "musicfm.mean.v1", true)
            .unwrap()
            .is_none());
        assert!(!store
            .delete_entity_embedding("track", "track1", "musicfm.mean.v1")
            .unwrap());
    }

    #[test]
    fn test_entity_embedding_unique_namespace_overwrites_existing_row() {
        let (store, _temp_dir) = create_test_store();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track1",
                "ast.instruments.v1",
                vec![1.0, 0.0],
            ))
            .unwrap();
        let updated = EntityEmbeddingUpsert {
            metadata: serde_json::json!({"source": "updated"}),
            model: serde_json::json!({"name": "test-model", "version": "v2"}),
            ..sample_embedding("track1", "ast.instruments.v1", vec![0.0, 1.0, 0.0])
        };
        let stored = store.upsert_entity_embedding(&updated).unwrap();

        assert_eq!(stored.dim, 3);
        assert_eq!(stored.vector.as_ref().unwrap(), &vec![0.0, 1.0, 0.0]);
        assert_eq!(stored.metadata["source"], "updated");
        assert_eq!(stored.model["version"], "v2");
        assert!(stored.updated_at >= stored.created_at);

        let listed = store
            .list_entity_embeddings("track", "track1", true)
            .unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn test_entity_embedding_multiple_namespaces_per_entity() {
        let (store, _temp_dir) = create_test_store();
        store
            .upsert_entity_embedding(&sample_embedding("track1", "ast.instruments.v1", vec![1.0]))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding("track1", "musicfm.mean.v1", vec![2.0]))
            .unwrap();

        let listed = store
            .list_entity_embeddings("track", "track1", false)
            .unwrap();
        let namespaces: Vec<_> = listed.iter().map(|item| item.namespace.as_str()).collect();
        assert_eq!(namespaces, vec!["ast.instruments.v1", "musicfm.mean.v1"]);
    }

    #[test]
    fn test_entity_embeddings_support_album_and_artist_entity_types() {
        let (store, _temp_dir) = create_test_store();
        let album = EntityEmbeddingUpsert {
            entity_type: "album".to_string(),
            entity_id: "album1".to_string(),
            namespace: "album.musicfm.median.v1".to_string(),
            vector: vec![1.0, 0.0],
            dtype: "float32".to_string(),
            metadata: serde_json::json!({"derived": true}),
            model: serde_json::json!({"id": "pezzottify-derived-album-embeddings"}),
        };
        let artist = EntityEmbeddingUpsert {
            entity_type: "artist".to_string(),
            entity_id: "artist1".to_string(),
            namespace: "artist.fact.genres.v1".to_string(),
            vector: vec![0.0, 1.0],
            dtype: "float32".to_string(),
            metadata: serde_json::json!({"source": "unit-test"}),
            model: serde_json::json!({"id": "artist-fact-test"}),
        };

        store.upsert_entity_embedding(&album).unwrap();
        store.upsert_entity_embedding(&artist).unwrap();

        let listed_album = store
            .list_entity_embeddings("album", "album1", false)
            .unwrap();
        assert_eq!(listed_album.len(), 1);
        assert_eq!(listed_album[0].namespace, "album.musicfm.median.v1");
        let listed_artist = store
            .list_entity_embeddings("artist", "artist1", false)
            .unwrap();
        assert_eq!(listed_artist.len(), 1);
        assert_eq!(listed_artist[0].namespace, "artist.fact.genres.v1");

        let album_results = store
            .search_entity_embeddings("album.musicfm.median.v1", &[1.0, 0.0], Some("album"), 10)
            .unwrap();
        assert_eq!(album_results.len(), 1);
        assert_eq!(album_results[0].entity_type, "album");
        assert_eq!(album_results[0].entity_id, "album1");

        let artist_results = store
            .search_entity_embeddings("artist.fact.genres.v1", &[0.0, 1.0], Some("artist"), 10)
            .unwrap();
        assert_eq!(artist_results.len(), 1);
        assert_eq!(artist_results[0].entity_type, "artist");
        assert_eq!(artist_results[0].entity_id, "artist1");
    }

    #[test]
    fn test_list_available_tracks_missing_embeddings_selects_incomplete_tracks() {
        let (store, _temp_dir) = create_test_store();
        seed_available_track(&store, "track_missing_all", Some("a.mp3"), true);
        seed_available_track(&store, "track_missing_ast", Some("b.mp3"), true);
        seed_available_track(&store, "track_complete", Some("c.mp3"), true);
        seed_available_track(&store, "track_unavailable", Some("d.mp3"), false);
        seed_available_track(&store, "track_no_audio", None, true);

        store
            .upsert_entity_embedding(&sample_embedding(
                "track_missing_ast",
                "musicfm.mean.v1",
                vec![1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track_complete",
                "musicfm.mean.v1",
                vec![1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track_complete",
                "ast.audioset.v1",
                vec![1.0],
            ))
            .unwrap();

        let tracks = store
            .list_available_tracks_missing_embeddings(
                &["musicfm.mean.v1".to_string(), "ast.audioset.v1".to_string()],
                10,
            )
            .unwrap();

        assert_eq!(
            tracks,
            vec![
                ("track_missing_all".to_string(), "a.mp3".to_string()),
                ("track_missing_ast".to_string(), "b.mp3".to_string()),
            ]
        );
    }

    #[test]
    fn test_list_available_tracks_missing_embeddings_honors_limit_and_empty_input() {
        let (store, _temp_dir) = create_test_store();
        seed_available_track(&store, "track_a", Some("a.mp3"), true);
        seed_available_track(&store, "track_b", Some("b.mp3"), true);

        assert!(store
            .list_available_tracks_missing_embeddings(&[], 10)
            .unwrap()
            .is_empty());
        assert!(store
            .list_available_tracks_missing_embeddings(&["musicfm.mean.v1".to_string()], 0)
            .unwrap()
            .is_empty());

        let tracks = store
            .list_available_tracks_missing_embeddings(&["musicfm.mean.v1".to_string()], 1)
            .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0], ("track_a".to_string(), "a.mp3".to_string()));
    }

    #[test]
    fn test_complete_album_tracklist_pages_are_bounded_and_use_keyset_cursor() {
        let (store, _temp_dir) = create_test_store();
        {
            let conn = store.write_conn.lock().unwrap();
            for (album_id, availability) in [
                ("album_a", "complete"),
                ("album_b", "partial"),
                ("album_c", "complete"),
            ] {
                conn.execute(
                    "INSERT INTO albums
                        (id, name, album_type, label, popularity, release_date,
                         release_date_precision, album_availability)
                     VALUES (?1, ?1, 'album', '', 0, '2024', 'year', ?2)",
                    params![album_id, availability],
                )
                .unwrap();
                for track_number in 1..=2 {
                    conn.execute(
                        "INSERT INTO tracks
                            (id, name, album_rowid, track_number, popularity, disc_number,
                             duration_ms, explicit, audio_uri, track_available)
                         VALUES (?1, ?1,
                            (SELECT rowid FROM albums WHERE id = ?2), ?3, 0, 1,
                            1000, 0, ?4, 1)",
                        params![
                            format!("{album_id}_track_{track_number}"),
                            album_id,
                            track_number,
                            format!("{album_id}_{track_number}.ogg")
                        ],
                    )
                    .unwrap();
                }
            }
        }

        let first = store.list_complete_album_tracklists_page(None, 1).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].album_id, "album_a");
        assert_eq!(first[0].tracks.len(), 2);

        let second = store
            .list_complete_album_tracklists_page(Some(first[0].album_rowid), 1)
            .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].album_id, "album_c");

        assert!(store
            .list_complete_album_tracklists_page(Some(second[0].album_rowid), 1)
            .unwrap()
            .is_empty());
        assert!(store
            .list_complete_album_tracklists_page(None, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_complete_album_page_candidate_plan_uses_pagination_index() {
        let (store, _temp_dir) = create_test_store();
        let conn = store.write_conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT rowid, id
                 FROM albums INDEXED BY idx_albums_availability
                 WHERE album_availability = 'complete'
                   AND rowid > ?1
                 ORDER BY rowid
                 LIMIT ?2",
            )
            .unwrap();
        let details = stmt
            .query_map(params![0, 100], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_albums_availability")),
            "unexpected query plan: {details:?}"
        );
        assert!(
            details
                .iter()
                .all(|detail| !detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "candidate selection must not globally sort: {details:?}"
        );
    }

    #[test]
    fn test_get_track_embedding_coverage_counts_per_namespace_and_fully_embedded() {
        let (store, _temp_dir) = create_test_store();
        seed_available_track(&store, "track_missing_all", Some("a.mp3"), true);
        seed_available_track(&store, "track_missing_ast", Some("b.mp3"), true);
        seed_available_track(&store, "track_complete", Some("c.mp3"), true);
        seed_available_track(&store, "track_unavailable", Some("d.mp3"), false);
        seed_available_track(&store, "track_no_audio", None, true);

        store
            .upsert_entity_embedding(&sample_embedding(
                "track_missing_ast",
                "musicfm.mean.v1",
                vec![1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track_complete",
                "musicfm.mean.v1",
                vec![1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track_complete",
                "ast.audioset.v1",
                vec![1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "track_unavailable",
                "musicfm.mean.v1",
                vec![1.0],
            ))
            .unwrap();

        let coverage = store
            .get_track_embedding_coverage(&[
                "musicfm.mean.v1".to_string(),
                "ast.audioset.v1".to_string(),
            ])
            .unwrap();

        assert_eq!(coverage.available_tracks, 3);
        assert_eq!(coverage.fully_embedded_tracks, 1);
        assert_eq!(coverage.tracks_missing_any_embedding, 2);
        assert_eq!(
            coverage.namespaces,
            vec![
                crate::catalog_store::TrackEmbeddingNamespaceCoverage {
                    namespace: "musicfm.mean.v1".to_string(),
                    embedded_tracks: 2,
                    missing_tracks: 1,
                },
                crate::catalog_store::TrackEmbeddingNamespaceCoverage {
                    namespace: "ast.audioset.v1".to_string(),
                    embedded_tracks: 1,
                    missing_tracks: 2,
                },
            ]
        );
    }

    #[test]
    fn test_get_album_embedding_coverage_counts_complete_local_albums() {
        let (store, temp_dir) = create_test_store();
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO artists (id, name, followers_total, popularity, artist_available, mbid_lookup_status)
                 VALUES ('artist1', 'Artist 1', 0, 0, 0, 0)",
                [],
            )
            .unwrap();
            for (album_id, availability) in [
                ("complete_album", "complete"),
                ("uri_only_album", "complete"),
                ("missing_uri_album", "partial"),
            ] {
                conn.execute(
                    "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
                     VALUES (?1, ?2, 'album', '', 0, '2024', 'year', ?3)",
                    params![album_id, album_id, availability],
                )
                .unwrap();
            }
            for (album_id, track_id, audio_uri) in [
                ("complete_album", "track1", Some("a.ogg")),
                ("complete_album", "track2", Some("b.ogg")),
                ("uri_only_album", "track3", Some("c.ogg")),
                ("missing_uri_album", "track4", None),
            ] {
                conn.execute(
                    "INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, audio_uri, track_available)
                     VALUES (?1, ?1, (SELECT rowid FROM albums WHERE id=?2), 1, 0, 1, 1000, 0, ?3, 1)",
                    params![track_id, album_id, audio_uri],
                )
                .unwrap();
            }
        }
        std::fs::write(temp_dir.path().join("a.ogg"), b"a").unwrap();
        std::fs::write(temp_dir.path().join("b.ogg"), b"b").unwrap();
        store
            .upsert_entity_embedding(&EntityEmbeddingUpsert {
                entity_type: "album".to_string(),
                entity_id: "complete_album".to_string(),
                namespace: "album.musicfm.median.v1".to_string(),
                vector: vec![1.0],
                dtype: "float32".to_string(),
                metadata: serde_json::json!({"derived": true}),
                model: serde_json::json!({"id": "pezzottify-derived-album-embeddings"}),
            })
            .unwrap();

        let coverage = store
            .get_album_embedding_coverage(&["album.musicfm.median.v1".to_string()], temp_dir.path())
            .unwrap();

        assert_eq!(coverage.complete_local_albums, 2);
        assert_eq!(
            coverage.namespaces,
            vec![crate::catalog_store::AlbumEmbeddingNamespaceCoverage {
                namespace: "album.musicfm.median.v1".to_string(),
                embedded_albums: 1,
                missing_albums: 1,
            }]
        );
    }

    #[test]
    fn test_entity_embedding_search_orders_by_cosine_similarity() {
        let (store, _temp_dir) = create_test_store();
        store
            .upsert_entity_embedding(&sample_embedding("close", "test.space.v1", vec![1.0, 0.0]))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "diagonal",
                "test.space.v1",
                vec![1.0, 1.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "opposite",
                "test.space.v1",
                vec![-1.0, 0.0],
            ))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding("other", "other.space.v1", vec![1.0, 0.0]))
            .unwrap();

        let results = store
            .search_entity_embeddings("test.space.v1", &[1.0, 0.0], Some("track"), 10)
            .unwrap();
        let ids: Vec<_> = results.iter().map(|item| item.entity_id.as_str()).collect();
        assert_eq!(ids, vec!["close", "diagonal", "opposite"]);
        assert!((results[0].score - 1.0).abs() < 1e-6);
        assert!(results[1].score > 0.70 && results[1].score < 0.72);
        assert!((results[2].score + 1.0).abs() < 1e-6);

        let limited = store
            .search_entity_embeddings("test.space.v1", &[1.0, 0.0], None, 2)
            .unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_entity_embedding_search_skips_dimension_mismatch() {
        let (store, _temp_dir) = create_test_store();
        store
            .upsert_entity_embedding(&sample_embedding("two-d", "mixed.v1", vec![1.0, 0.0]))
            .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding(
                "three-d",
                "mixed.v1",
                vec![1.0, 0.0, 0.0],
            ))
            .unwrap();

        let results = store
            .search_entity_embeddings("mixed.v1", &[1.0, 0.0], None, 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id, "two-d");
    }

    #[test]
    fn test_entity_embedding_rejects_empty_and_unsupported_dtype() {
        let (store, _temp_dir) = create_test_store();
        let empty = sample_embedding("track1", "bad.v1", vec![]);
        assert!(store.upsert_entity_embedding(&empty).is_err());

        let bad_dtype = EntityEmbeddingUpsert {
            dtype: "float16".to_string(),
            ..sample_embedding("track1", "bad.v1", vec![1.0])
        };
        assert!(store.upsert_entity_embedding(&bad_dtype).is_err());

        assert!(store
            .search_entity_embeddings("bad.v1", &[], None, 10)
            .is_err());
        assert!(store
            .search_entity_embeddings("bad.v1", &[0.0, 0.0], None, 10)
            .is_err());
    }

    #[test]
    fn test_catalog_migration_v5_to_v6_adds_entity_embeddings() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("migration.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            CATALOG_VERSIONED_SCHEMAS[5].create(&conn).unwrap();
            conn.pragma_update(
                None,
                "user_version",
                crate::sqlite_persistence::BASE_DB_VERSION + 5,
            )
            .unwrap();
        }

        let store = SqliteCatalogStore::new(
            &db_path,
            temp_dir.path(),
            1,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        store
            .upsert_entity_embedding(&sample_embedding("track1", "musicfm.mean.v1", vec![1.0]))
            .unwrap();
        let stored = store
            .get_entity_embedding("track", "track1", "musicfm.mean.v1", true)
            .unwrap();
        assert!(stored.is_some());
    }

    #[test]
    fn test_refresh_availability_fast_path_returns_persisted_totals_and_indexed_counts() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = SqliteCatalogStore::new(
            temp_dir.path().join("test.db"),
            temp_dir.path(),
            1,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        std::fs::create_dir_all(temp_dir.path().join("audio")).unwrap();
        std::fs::write(temp_dir.path().join("audio/track1.ogg"), b"audio").unwrap();

        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute_batch(
                "INSERT INTO artists
                     (id, name, followers_total, popularity, artist_available, mbid_lookup_status)
                 VALUES ('artist1', 'Artist 1', 0, 0, 1, 0);
                 INSERT INTO albums
                     (id, name, album_type, label, popularity, release_date,
                      release_date_precision, album_availability)
                 VALUES ('album1', 'Album 1', 'album', '', 0, '2024', 'year', 'partial');
                 INSERT INTO tracks
                     (id, name, album_rowid, track_number, popularity, disc_number,
                      duration_ms, explicit, audio_uri, track_available)
                 VALUES
                     ('track1', 'Track 1', (SELECT rowid FROM albums WHERE id='album1'),
                      1, 0, 1, 1000, 0, 'audio/track1.ogg', 1),
                     ('track2', 'Track 2', (SELECT rowid FROM albums WHERE id='album1'),
                      2, 0, 1, 1000, 0, NULL, 0);",
            )
            .unwrap();
        }

        let refresh = store.refresh_availability_and_stats().unwrap();

        assert_eq!(refresh.repaired.tracks_updated, 0);
        assert_eq!(refresh.repaired.albums_updated, 0);
        assert_eq!(refresh.repaired.artists_updated, 0);
        assert_eq!(refresh.stats.artists.total, 1);
        assert_eq!(refresh.stats.artists.available, 1);
        assert_eq!(refresh.stats.albums.total, 1);
        assert_eq!(refresh.stats.albums.available, 1);
        assert_eq!(refresh.stats.tracks.total, 2);
        assert_eq!(refresh.stats.tracks.available, 1);
        assert_eq!(refresh.stats.tracks.unavailable, 1);
    }

    #[test]
    fn test_refresh_availability_respects_cancellation() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = SqliteCatalogStore::new(
            temp_dir.path().join("test.db"),
            temp_dir.path(),
            1,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();

        // Seed one minimal track graph so refresh work would normally proceed.
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity, artist_available, mbid_lookup_status)
                 VALUES ('artist1', 'Artist 1', 0, 0, 0, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
                 VALUES ('album1', 'Album 1', 'album', '', 0, '2024', 'year', 'missing')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, track_available)
                 VALUES ('track1', 'Track 1', (SELECT rowid FROM albums WHERE id='album1'), 1, 0, 1, 1000, 0, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO track_artists (track_rowid, artist_rowid, role)
                 VALUES ((SELECT rowid FROM tracks WHERE id='track1'), (SELECT rowid FROM artists WHERE id='artist1'), 0)",
                [],
            )
            .unwrap();
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        cancelled.store(true, Ordering::SeqCst);

        let result =
            store.refresh_availability_and_stats_with_cancel(&|| cancelled.load(Ordering::SeqCst));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err.contains("cancelled"),
            "Expected cancellation error, got: {}",
            err
        );
    }

    #[test]
    fn test_refresh_availability_mid_run_cancellation_rolls_back() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = SqliteCatalogStore::new(
            temp_dir.path().join("test.db"),
            temp_dir.path(),
            1,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();

        // Seed enough rows to ensure cancellation can happen during processing.
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity, artist_available, mbid_lookup_status)
                 VALUES ('artist1', 'Artist 1', 0, 0, 0, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
                 VALUES ('album1', 'Album 1', 'album', '', 0, '2024', 'year', 'missing')",
                [],
            )
            .unwrap();

            for i in 0..5000 {
                let track_id = format!("track{}", i);
                let track_name = format!("Track {}", i);
                conn.execute(
                    "INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit, track_available)
                     VALUES (?1, ?2, (SELECT rowid FROM albums WHERE id='album1'), ?3, 0, 1, 1000, 0, 1)",
                    params![track_id, track_name, i + 1],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO track_artists (track_rowid, artist_rowid, role)
                     VALUES ((SELECT rowid FROM tracks WHERE id=?1), (SELECT rowid FROM artists WHERE id='artist1'), 0)",
                    params![format!("track{}", i)],
                )
                .unwrap();
            }
        }

        let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let checks_clone = checks.clone();
        let result = store.refresh_availability_and_stats_with_cancel(&|| {
            let n = checks_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            n > 300
        });
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err.contains("cancelled"),
            "Expected cancellation error, got: {}",
            err
        );

        // Ensure transaction rollback happened: all seeded tracks stay available=1.
        let conn = store.write_conn.lock().unwrap();
        let still_available: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE track_available = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_available, 5000);
    }
}
