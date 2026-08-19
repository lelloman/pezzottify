impl user_store::UserEventStore for SqliteUserStore {
    fn append_event(
        &self,
        user_id: usize,
        event: &crate::user::sync_events::UserEvent,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        SqliteUserStore::append_event(self, user_id, event)
    }

    fn get_events_since(
        &self,
        user_id: usize,
        since_seq: i64,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        SqliteUserStore::get_events_since(self, user_id, since_seq)
    }

    fn get_current_seq(&self, user_id: usize) -> Result<i64> {
        SqliteUserStore::get_current_seq(self, user_id)
    }

    fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>> {
        SqliteUserStore::get_min_seq(self, user_id)
    }

    fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64> {
        SqliteUserStore::prune_events_older_than(self, before_timestamp)
    }

    fn set_user_role_with_event(
        &self,
        user_id: usize,
        role: UserRole,
        enabled: bool,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut roles: Vec<UserRole> = existing
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .filter_map(|value| UserRole::from_str(value.trim()))
            .collect();

        if enabled && !roles.contains(&role) {
            roles.push(role);
        } else if !enabled {
            roles.retain(|existing| existing != &role);
        }

        if roles.is_empty() {
            tx.execute(
                &format!(
                    "DELETE FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
            )?;
        } else {
            let roles = roles
                .iter()
                .map(|role| role.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if existing.is_some() {
                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![roles, user_id],
                )?;
            } else {
                tx.execute(
                    &format!(
                        "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![user_id, roles],
                )?;
            }
        }

        let event = UserEvent::PermissionsReset {
            permissions: resolve_permissions(&tx, user_id)?,
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn add_extra_permission_with_event(
        &self,
        user_id: usize,
        grant: PermissionGrant,
    ) -> Result<(usize, crate::user::sync_events::StoredEvent)> {
        let PermissionGrant::Extra {
            start_time,
            end_time,
            permission,
            countdown,
        } = grant
        else {
            bail!("Cannot add ByRole grant as extra permission");
        };

        let start_time = start_time.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;
        let end_time = end_time
            .map(|value| value.duration_since(SystemTime::UNIX_EPOCH))
            .transpose()?
            .map(|duration| duration.as_secs() as i64);
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO {} (user_id, permission, start_time, end_time, countdown)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![
                user_id,
                permission.as_int(),
                start_time,
                end_time,
                countdown.map(|value| value as i64)
            ],
        )?;
        let permission_id = tx.last_insert_rowid() as usize;
        let event = UserEvent::PermissionGranted { permission };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok((permission_id, stored))
    }

    fn remove_extra_permission_with_event(
        &self,
        permission_id: usize,
    ) -> Result<Option<(usize, Permission, crate::user::sync_events::StoredEvent)>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing: Option<(usize, i32)> = tx
            .query_row(
                &format!(
                    "SELECT user_id, permission FROM {} WHERE id = ?1",
                    USER_EXTRA_PERMISSION_TABLE_V_4.name
                ),
                params![permission_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((user_id, permission)) = existing else {
            tx.commit()?;
            return Ok(None);
        };
        let permission = Permission::from_int(permission)
            .ok_or_else(|| anyhow::anyhow!("Invalid permission int: {permission}"))?;
        tx.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
        )?;
        let event = UserEvent::PermissionRevoked { permission };
        let stored = Self::append_event_tx(&tx, user_id, &event, None, 0)?;
        tx.commit()?;
        Ok(Some((user_id, permission, stored)))
    }

    fn set_liked_content_with_event(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
        operation_id: Option<&str>,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let event = if liked {
            crate::user::sync_events::UserEvent::ContentLiked {
                content_type,
                content_id: content_id.to_owned(),
            }
        } else {
            crate::user::sync_events::UserEvent::ContentUnliked {
                content_type,
                content_id: content_id.to_owned(),
            }
        };
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if existing.event != event {
                bail!("Operation id was already used for a different mutation");
            }
            tx.commit()?;
            return Ok(existing);
        }

        if liked {
            tx.execute(
                "INSERT OR IGNORE INTO liked_content (user_id, content_id, content_type)
                 VALUES (?1, ?2, ?3)",
                params![user_id, content_id, content_type.as_int()],
            )?;
        } else {
            tx.execute(
                "DELETE FROM liked_content WHERE user_id = ?1 AND content_id = ?2",
                params![user_id, content_id],
            )?;
        }
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn create_playlist_with_event(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_id: usize,
        track_ids: Vec<String>,
        operation_id: Option<&str>,
    ) -> Result<(String, crate::user::sync_events::StoredEvent)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if let crate::user::sync_events::UserEvent::PlaylistCreated { playlist_id, .. } =
                &existing.event
            {
                if !matches!(
                    &existing.event,
                    crate::user::sync_events::UserEvent::PlaylistCreated { name, .. }
                        if name == playlist_name
                ) {
                    return Err(super::UserServiceError::operation_conflict().into());
                }
                let playlist_id = playlist_id.clone();
                tx.commit()?;
                return Ok((playlist_id, existing));
            }
            return Err(super::UserServiceError::operation_conflict().into());
        }

        let mut playlist_id = random_string(16);
        while tx.query_row(
            "SELECT COUNT(*) FROM user_playlist WHERE id = ?1",
            params![playlist_id],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            playlist_id = random_string(16);
        }
        tx.execute(
            "INSERT INTO user_playlist (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
            params![&playlist_id, user_id, playlist_name, creator_id],
        )?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO user_playlist_tracks (playlist_id, track_id, position)
                 VALUES (?1, ?2, ?3)",
                params![&playlist_id, track_id, position as i32],
            )?;
        }
        let event = crate::user::sync_events::UserEvent::PlaylistCreated {
            playlist_id: playlist_id.clone(),
            name: playlist_name.to_owned(),
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok((playlist_id, stored))
    }

    fn update_playlist_with_events(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
        operation_id: Option<&str>,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing = Self::get_operation_events_tx(&tx, user_id, operation_id)?;
        if !existing.is_empty() {
            let expected: Vec<_> = playlist_name
                .iter()
                .map(
                    |name| crate::user::sync_events::UserEvent::PlaylistRenamed {
                        playlist_id: playlist_id.to_owned(),
                        name: name.clone(),
                    },
                )
                .chain(track_ids.iter().map(|tracks| {
                    crate::user::sync_events::UserEvent::PlaylistTracksUpdated {
                        playlist_id: playlist_id.to_owned(),
                        track_ids: tracks.clone(),
                    }
                }))
                .collect();
            if existing
                .iter()
                .map(|event| &event.event)
                .ne(expected.iter())
            {
                return Err(super::UserServiceError::operation_conflict().into());
            }
            tx.commit()?;
            return Ok(existing);
        }
        let owner: Option<usize> = tx
            .query_row(
                "SELECT user_id FROM user_playlist WHERE id = ?1",
                params![playlist_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(owner) = owner else {
            return Err(super::UserServiceError::playlist_not_found().into());
        };
        if owner != user_id {
            return Err(super::UserServiceError::playlist_not_found().into());
        }

        let mut events = Vec::new();
        if let Some(name) = playlist_name {
            tx.execute(
                "UPDATE user_playlist SET name = ?1 WHERE id = ?2",
                params![&name, playlist_id],
            )?;
            let event = crate::user::sync_events::UserEvent::PlaylistRenamed {
                playlist_id: playlist_id.to_owned(),
                name,
            };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        if let Some(track_ids) = track_ids {
            tx.execute(
                "DELETE FROM user_playlist_tracks WHERE playlist_id = ?1",
                params![playlist_id],
            )?;
            for (position, track_id) in track_ids.iter().enumerate() {
                tx.execute(
                    "INSERT INTO user_playlist_tracks (playlist_id, track_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![playlist_id, track_id, position as i32],
                )?;
            }
            let event = crate::user::sync_events::UserEvent::PlaylistTracksUpdated {
                playlist_id: playlist_id.to_owned(),
                track_ids,
            };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        tx.commit()?;
        Ok(events)
    }

    fn delete_playlist_with_event(
        &self,
        playlist_id: &str,
        user_id: usize,
        operation_id: Option<&str>,
    ) -> Result<crate::user::sync_events::StoredEvent> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(existing) = Self::get_operation_events_tx(&tx, user_id, operation_id)?
            .into_iter()
            .next()
        {
            if !matches!(
                &existing.event,
                crate::user::sync_events::UserEvent::PlaylistDeleted { playlist_id: id }
                    if id == playlist_id
            ) {
                return Err(super::UserServiceError::operation_conflict().into());
            }
            tx.commit()?;
            return Ok(existing);
        }
        let changed = tx.execute(
            "DELETE FROM user_playlist WHERE id = ?1 AND user_id = ?2",
            params![playlist_id, user_id],
        )?;
        if changed == 0 {
            return Err(super::UserServiceError::playlist_not_found().into());
        }
        let event = crate::user::sync_events::UserEvent::PlaylistDeleted {
            playlist_id: playlist_id.to_owned(),
        };
        let stored = Self::append_event_tx(&tx, user_id, &event, operation_id, 0)?;
        tx.commit()?;
        Ok(stored)
    }

    fn set_settings_with_events(
        &self,
        user_id: usize,
        settings: Vec<UserSetting>,
        operation_id: Option<&str>,
    ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let existing = Self::get_operation_events_tx(&tx, user_id, operation_id)?;
        if !existing.is_empty() {
            let expected: Vec<_> = settings
                .iter()
                .cloned()
                .map(|setting| crate::user::sync_events::UserEvent::SettingChanged { setting })
                .collect();
            if existing
                .iter()
                .map(|event| &event.event)
                .ne(expected.iter())
            {
                bail!("Operation id was already used for a different mutation");
            }
            tx.commit()?;
            return Ok(existing);
        }
        let mut events = Vec::with_capacity(settings.len());
        for setting in settings {
            tx.execute(
                "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
                 VALUES (?1, ?2, ?3, (cast(strftime('%s','now') as int)))
                 ON CONFLICT(user_id, setting_key) DO UPDATE SET
                    setting_value = excluded.setting_value, updated = excluded.updated",
                params![user_id, setting.key(), setting.value_to_string()],
            )?;
            let event = crate::user::sync_events::UserEvent::SettingChanged { setting };
            events.push(Self::append_event_tx(
                &tx,
                user_id,
                &event,
                operation_id,
                events.len() as i32,
            )?);
        }
        tx.commit()?;
        Ok(events)
    }

    fn get_sync_snapshot(
        &self,
        user_id: usize,
    ) -> Result<crate::user::sync_events::UserSyncSnapshot> {
        use std::collections::HashSet;

        fn liked(
            tx: &Transaction<'_>,
            user_id: usize,
            content_type: LikedContentType,
        ) -> Result<Vec<String>> {
            let mut stmt = tx.prepare(
                "SELECT content_id FROM liked_content
                 WHERE user_id = ?1 AND content_type = ?2",
            )?;
            let result = stmt
                .query_map(params![user_id, content_type.as_int()], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(result)
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let seq = tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM user_events WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )?;
        let liked_albums = liked(&tx, user_id, LikedContentType::Album)?;
        let liked_artists = liked(&tx, user_id, LikedContentType::Artist)?;
        let liked_tracks = liked(&tx, user_id, LikedContentType::Track)?;

        let settings = {
            let mut stmt = tx.prepare(
                "SELECT setting_key, setting_value FROM user_settings WHERE user_id = ?1",
            )?;
            let result = stmt
                .query_map(params![user_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|row| row.ok())
                .filter_map(|(key, value)| UserSetting::from_key_value(&key, &value).ok())
                .collect();
            result
        };

        let playlists = {
            let mut stmt = tx.prepare(
                "SELECT p.id, p.user_id, u.handle, p.name, p.created
                 FROM user_playlist p
                 JOIN user u ON u.id = p.creator_id
                 WHERE p.user_id = ?1",
            )?;
            let rows = stmt
                .query_map(params![user_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, usize>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut playlists = Vec::with_capacity(rows.len());
            for (id, owner_id, creator, name, created) in rows {
                let mut tracks_stmt = tx.prepare(
                    "SELECT track_id FROM user_playlist_tracks
                     WHERE playlist_id = ?1 ORDER BY position ASC",
                )?;
                let tracks = tracks_stmt
                    .query_map(params![&id], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                playlists.push(UserPlaylist {
                    id,
                    user_id: owner_id,
                    creator,
                    name,
                    created: system_time_from_column_result(created),
                    tracks,
                });
            }
            playlists
        };

        let mut permissions = HashSet::new();
        let roles: Option<String> = tx
            .query_row(
                "SELECT role FROM user_role WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(roles) = roles {
            for role in roles
                .split(',')
                .filter_map(|role| UserRole::from_str(role.trim()))
            {
                permissions.extend(role.permissions().iter().copied());
            }
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs() as i64;
        {
            let mut stmt = tx.prepare(
                "SELECT permission FROM user_extra_permission
                 WHERE user_id = ?1 AND start_time <= ?2
                   AND (end_time IS NULL OR end_time >= ?2)
                   AND (countdown IS NULL OR countdown > 0)",
            )?;
            for value in stmt
                .query_map(params![user_id, now], |row| row.get::<_, i32>(0))?
                .filter_map(|value| value.ok())
            {
                if let Some(permission) = Permission::from_int(value) {
                    permissions.insert(permission);
                }
            }
        }

        let notifications = {
            let mut stmt = tx.prepare(
                "SELECT id, notification_type, title, body, data, read_at, created_at
                 FROM user_notifications WHERE user_id = ?1
                 ORDER BY created_at DESC, rowid DESC",
            )?;
            let raw = stmt
                .query_map(params![user_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            raw.into_iter()
                .map(|(id, kind, title, body, data, read_at, created_at)| {
                    Ok(crate::notifications::Notification {
                        id,
                        notification_type: serde_json::from_str(&kind)?,
                        title,
                        body,
                        data: serde_json::from_str(&data)?,
                        read_at,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>>>()?
        };

        tx.commit()?;
        Ok(crate::user::sync_events::UserSyncSnapshot {
            seq,
            liked_albums,
            liked_artists,
            liked_tracks,
            settings,
            playlists,
            permissions: permissions.into_iter().collect(),
            notifications,
        })
    }
}

fn resolve_permissions(conn: &Connection, user_id: usize) -> Result<Vec<Permission>> {
    let mut permissions = HashSet::new();
    let roles: Option<String> = conn
        .query_row(
            &format!(
                "SELECT role FROM {} WHERE user_id = ?1",
                USER_ROLE_TABLE_V_4.name
            ),
            params![user_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(roles) = roles {
        for role in roles
            .split(',')
            .filter_map(|value| UserRole::from_str(value.trim()))
        {
            permissions.extend(role.permissions().iter().copied());
        }
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as i64;
    let mut stmt = conn.prepare(&format!(
        "SELECT permission FROM {} WHERE user_id = ?1 AND start_time <= ?2
         AND (end_time IS NULL OR end_time >= ?2)
         AND (countdown IS NULL OR countdown > 0)",
        USER_EXTRA_PERMISSION_TABLE_V_4.name
    ))?;
    for value in stmt
        .query_map(params![user_id, now], |row| row.get::<_, i32>(0))?
        .filter_map(|value| value.ok())
    {
        if let Some(permission) = Permission::from_int(value) {
            permissions.insert(permission);
        }
    }
    Ok(permissions.into_iter().collect())
}

