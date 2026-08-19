impl UserStore for SqliteUserStore {
    fn create_user(&self, user_handle: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO user (handle) VALUES (?1)",
            params![user_handle],
        )
        .with_context(|| format!("Failed to create user {}", user_handle))?;

        Ok(conn.last_insert_rowid() as usize)
    }

    fn delete_user(&self, user_id: usize) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            &format!("DELETE FROM {} WHERE id = ?1", USER_TABLE_V_0.name),
            params![user_id],
        )?;
        Ok(rows_affected > 0)
    }

    fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, name FROM {} WHERE user_id = ?1",
            USER_PLAYLIST_TABLE_V_3.name
        ))?;
        let playlists = stmt
            .query_map(params![user_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(playlists)
    }

    fn get_user_handle(&self, user_id: usize) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT handle FROM {} WHERE id = ?1",
            USER_TABLE_V_0.name
        ))?;
        match stmt.query_row(params![user_id], |row| row.get(0)) {
            Ok(handle) => Ok(Some(handle)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_all_user_handles(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("SELECT handle FROM {}", USER_TABLE_V_0.name))?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(rows)
    }

    fn get_user_id(&self, user_handle: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id FROM {} WHERE handle = ?1",
            USER_TABLE_V_0.name
        ))?;
        match stmt.query_row(params![user_handle], |row| row.get(0)) {
            Ok(id) => {
                let id: i32 = id;
                Ok(Some(id as usize))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_user_id_by_oidc_subject(&self, oidc_subject: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT id FROM {} WHERE oidc_subject = ?1",
            USER_TABLE_V_12.name
        ))?;
        match stmt.query_row(params![oidc_subject], |row| row.get(0)) {
            Ok(id) => {
                let id: i32 = id;
                Ok(Some(id as usize))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_user_oidc_subject(&self, user_id: usize, oidc_subject: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "UPDATE {} SET oidc_subject = ?1 WHERE id = ?2",
                USER_TABLE_V_12.name
            ),
            params![oidc_subject, user_id],
        )?;
        Ok(())
    }

    fn get_user_oidc_subject(&self, user_id: usize) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT oidc_subject FROM {} WHERE id = ?1",
            USER_TABLE_V_12.name
        ))?;
        match stmt.query_row(params![user_id], |row| row.get::<_, Option<String>>(0)) {
            Ok(subject) => Ok(subject),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn clear_user_oidc_subject(&self, user_id: usize) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "UPDATE {} SET oidc_subject = NULL WHERE id = ?1",
                USER_TABLE_V_12.name
            ),
            params![user_id],
        )?;
        Ok(())
    }

    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Result<Option<bool>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT COUNT(*) FROM {} WHERE user_id = ?1 AND content_id = ?2",
            LIKED_CONTENT_TABLE_V_2.name
        ))?;
        let count: i32 = stmt.query_row(params![user_id, content_id], |row| row.get(0))?;

        Ok(Some(count > 0))
    }

    fn set_user_liked_content(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if liked {
            conn.execute(
                &format!(
                    "INSERT OR IGNORE INTO {} (user_id, content_id, content_type) VALUES (?1, ?2, ?3)",
                    LIKED_CONTENT_TABLE_V_2.name
                ),
                params![user_id, content_id, content_type.as_int()],
            )?;
        } else {
            conn.execute(
                &format!(
                    "DELETE FROM {} WHERE user_id = ?1 AND content_id = ?2",
                    LIKED_CONTENT_TABLE_V_2.name
                ),
                params![user_id, content_id],
            )?;
        }

        Ok(())
    }

    fn get_user_liked_content(
        &self,
        user_id: usize,
        content_type: LikedContentType,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT content_id FROM {} WHERE user_id = ?1 AND content_type = ?2",
                LIKED_CONTENT_TABLE_V_2.name
            ))
            .ok()
            .unwrap();
        Ok(stmt
            .query_map(params![user_id, content_type.as_int()], |row| row.get(0))
            .ok()
            .unwrap()
            .collect::<Result<Vec<String>, _>>()?)
    }

    fn create_user_playlist(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_user_id: usize,
        track_ids: Vec<String>,
    ) -> Result<String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Generate a random 16 A-z0-9 string that's not already a playlist id
        let mut playlist_id = random_string(16);
        while tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE id = ?1",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id],
            |row| row.get::<usize, i64>(0),
        )? > 0
        {
            playlist_id = random_string(16);
        }

        tx.execute(
            &format!(
                "INSERT INTO {} (id, user_id, name, creator_id) VALUES (?1, ?2, ?3, ?4)",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![&playlist_id, user_id, playlist_name, creator_user_id],
        )
        .context("Could not create playlist")?;

        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                &format!(
                    "INSERT INTO {} (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                    USER_PLAYLIST_TRACKS_TABLE_V_3.name
                ),
                params![playlist_id, track_id, position as i32],
            )?;
        }

        tx.commit()?;
        Ok(playlist_id)
    }

    fn update_user_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let playlist_user_id = tx
            .query_row(
                &format!(
                    "SELECT user_id FROM {} WHERE id = ?1",
                    USER_PLAYLIST_TABLE_V_3.name
                ),
                params![playlist_id],
                |row| row.get::<usize, usize>(0),
            )
            .optional()?
            .ok_or_else(super::UserServiceError::playlist_not_found)?;
        debug!("update_user_playlist({playlist_id}) found user_id: {playlist_user_id}",);
        if user_id != playlist_user_id {
            return Err(super::UserServiceError::playlist_not_found().into());
        }

        if let Some(playlist_name) = playlist_name {
            debug!("update_user_playlist({playlist_id}) updating name to {playlist_name}",);
            tx.execute(
                &format!(
                    "UPDATE {} SET name = ?1 WHERE id = ?2",
                    USER_PLAYLIST_TABLE_V_3.name
                ),
                params![playlist_name, playlist_id],
            )?;
        }

        if let Some(track_ids) = track_ids {
            debug!("update_user_playlist({playlist_id}) updating tracks",);
            tx.execute(
                &format!(
                    "DELETE FROM {} WHERE playlist_id = ?1",
                    USER_PLAYLIST_TRACKS_TABLE_V_3.name
                ),
                params![playlist_id],
            )?;

            for (position, track_id) in track_ids.iter().enumerate() {
                tx.execute(
                    &format!(
                        "INSERT INTO {} (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                        USER_PLAYLIST_TRACKS_TABLE_V_3.name
                    ),
                    params![playlist_id, track_id, position as i32],
                )?;
            }
        }
        debug!("update_user_playlist({playlist_id}) committing...",);
        tx.commit()?;
        Ok(())
    }

    fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1 AND user_id = ?2",
                USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id, user_id],
        )?;
        if changed == 0 {
            return Err(super::UserServiceError::playlist_not_found().into());
        }
        Ok(())
    }

    fn get_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<UserPlaylist> {
        let conn = self.conn.lock().unwrap();

        debug!("get_user_playlist({playlist_id})");

        let creator_name = conn.query_row(
            &format!(
                "SELECT handle FROM {} WHERE id = (SELECT creator_id FROM {} WHERE id = ?1)",
                USER_TABLE_V_0.name, USER_PLAYLIST_TABLE_V_3.name
            ),
            params![playlist_id],
            |row| row.get(0),
        )?;
        debug!("get_user_playlist({playlist_id}) found creator name: {creator_name}",);

        let mut stmt = conn.prepare(&format!(
            "SELECT id, name, created FROM {} WHERE id = ?1 AND user_id = ?2",
            USER_PLAYLIST_TABLE_V_3.name
        ))?;
        let mut playlist = stmt.query_row(params![playlist_id, user_id], |row| {
            Ok(UserPlaylist {
                id: row.get(0)?,
                user_id,
                creator: creator_name,
                name: row.get(1)?,
                created: system_time_from_column_result(row.get(2)?),
                tracks: vec![],
            })
        })?;

        debug!("get_user_playlist({playlist_id}) fetching tracks...",);
        let track_ids = conn
            .prepare(&format!(
                "SELECT track_id FROM {} WHERE playlist_id = ?1 ORDER BY position",
                USER_PLAYLIST_TRACKS_TABLE_V_3.name
            ))?
            .query_map(params![playlist_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        playlist.tracks = track_ids;
        Ok(playlist)
    }

    fn get_user_roles(&self, user_id: usize) -> Result<Vec<UserRole>> {
        debug!("get_user_roles: querying roles for user_id={}", user_id);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT role FROM {} WHERE user_id = ?1",
            USER_ROLE_TABLE_V_4.name
        ))?;
        let roles = stmt
            .query_map(params![user_id], |row| {
                let role_str: String = row.get(0)?;
                debug!(
                    "get_user_roles: found role string '{}' for user_id={}",
                    role_str, user_id
                );
                Ok(role_str)
            })?
            .filter_map(|r| r.ok())
            .flat_map(|s| {
                s.split(',')
                    .map(|part| part.trim())
                    .filter_map(UserRole::from_str)
                    .collect::<Vec<_>>()
            })
            .collect();
        debug!(
            "get_user_roles: returning {:?} for user_id={}",
            roles, user_id
        );
        Ok(roles)
    }

    fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Try to get existing roles for this user
        let existing_roles: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse existing roles and check if this role is already present
            let mut roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(UserRole::from_str)
                .collect();

            if !roles.contains(&role) {
                roles.push(role);
                let roles_str = roles
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![roles_str, user_id],
                )?;
            }
        } else {
            // No existing roles, insert new row
            tx.execute(
                &format!(
                    "INSERT INTO {} (user_id, role) VALUES (?1, ?2)",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id, role.as_str()],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn remove_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get existing roles for this user
        let existing_roles: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT role FROM {} WHERE user_id = ?1",
                    USER_ROLE_TABLE_V_4.name
                ),
                params![user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing) = existing_roles {
            // Parse and filter out the role to remove
            let roles: Vec<UserRole> = existing
                .split(',')
                .map(|s| s.trim())
                .filter_map(UserRole::from_str)
                .filter(|r| r != &role)
                .collect();

            if roles.is_empty() {
                // No roles left, delete the row
                tx.execute(
                    &format!(
                        "DELETE FROM {} WHERE user_id = ?1",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![user_id],
                )?;
            } else {
                // Update with remaining roles
                let roles_str = roles
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(",");

                tx.execute(
                    &format!(
                        "UPDATE {} SET role = ?1 WHERE user_id = ?2",
                        USER_ROLE_TABLE_V_4.name
                    ),
                    params![roles_str, user_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn add_user_extra_permission(&self, user_id: usize, grant: PermissionGrant) -> Result<usize> {
        match grant {
            PermissionGrant::ByRole(_) => {
                bail!("Cannot add ByRole grant as extra permission");
            }
            PermissionGrant::Extra {
                start_time,
                end_time,
                permission,
                countdown,
            } => {
                let conn = self.conn.lock().unwrap();
                let start_time_secs = start_time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let end_time_secs = end_time
                    .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64);
                let countdown_i64 = countdown.map(|c| c as i64);

                conn.execute(
                    &format!(
                        "INSERT INTO {} (user_id, permission, start_time, end_time, countdown) VALUES (?1, ?2, ?3, ?4, ?5)",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![user_id, permission.as_int(), start_time_secs, end_time_secs, countdown_i64],
                )?;
                Ok(conn.last_insert_rowid() as usize)
            }
        }
    }

    fn remove_user_extra_permission(
        &self,
        permission_id: usize,
    ) -> Result<Option<(usize, Permission)>> {
        let conn = self.conn.lock().unwrap();

        // First, get the user_id and permission before deleting
        let result: Option<(usize, i32)> = conn
            .query_row(
                &format!(
                    "SELECT user_id, permission FROM {} WHERE id = ?1",
                    USER_EXTRA_PERMISSION_TABLE_V_4.name
                ),
                params![permission_id],
                |row| Ok((row.get::<_, usize>(0)?, row.get::<_, i32>(1)?)),
            )
            .ok();

        // Delete the permission
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
        )?;

        // Return the deleted info if found
        match result {
            Some((user_id, perm_int)) => {
                let permission = Permission::from_int(perm_int)
                    .ok_or_else(|| anyhow::anyhow!("Invalid permission int: {}", perm_int))?;
                Ok(Some((user_id, permission)))
            }
            None => Ok(None),
        }
    }

    fn decrement_permission_countdown(&self, permission_id: usize) -> Result<bool> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get current countdown
        let current_countdown: Option<i64> = tx.query_row(
            &format!(
                "SELECT countdown FROM {} WHERE id = ?1",
                USER_EXTRA_PERMISSION_TABLE_V_4.name
            ),
            params![permission_id],
            |row| row.get(0),
        )?;

        let result = match current_countdown {
            None => Ok(true), // No countdown, permission remains valid
            Some(count) if count <= 1 => {
                // Last use, delete the permission
                tx.execute(
                    &format!(
                        "DELETE FROM {} WHERE id = ?1",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![permission_id],
                )?;
                Ok(false)
            }
            Some(count) => {
                // Decrement the countdown
                tx.execute(
                    &format!(
                        "UPDATE {} SET countdown = ?1 WHERE id = ?2",
                        USER_EXTRA_PERMISSION_TABLE_V_4.name
                    ),
                    params![count - 1, permission_id],
                )?;
                Ok(true)
            }
        };

        tx.commit()?;
        result
    }

    fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>> {
        use std::collections::HashSet;

        debug!("resolve_user_permissions: starting for user_id={}", user_id);
        let mut permissions = HashSet::new();

        // Add permissions from roles
        let roles = self.get_user_roles(user_id)?;
        debug!(
            "resolve_user_permissions: user_id={} has roles: {:?}",
            user_id, roles
        );
        for role in &roles {
            let role_perms = role.permissions();
            debug!(
                "resolve_user_permissions: adding {:?} permissions from role {:?}",
                role_perms.len(),
                role
            );
            for permission in role_perms {
                permissions.insert(*permission);
            }
        }

        // Add active extra permissions
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        debug!(
            "resolve_user_permissions: checking extra permissions for user_id={} at timestamp={}",
            user_id, now
        );

        let mut stmt = conn.prepare(&format!(
            "SELECT permission FROM {} WHERE user_id = ?1 AND start_time <= ?2 AND (end_time IS NULL OR end_time >= ?2) AND (countdown IS NULL OR countdown > 0)",
            USER_EXTRA_PERMISSION_TABLE_V_4.name
        ))?;

        let extra_perms = stmt
            .query_map(params![user_id, now], |row| {
                let perm_int: i32 = row.get(0)?;
                Ok(perm_int)
            })?
            .filter_map(|r| r.ok().and_then(Permission::from_int))
            .collect::<Vec<_>>();

        debug!(
            "resolve_user_permissions: found {} extra permissions for user_id={}",
            extra_perms.len(),
            user_id
        );
        for perm in &extra_perms {
            debug!(
                "resolve_user_permissions: adding extra permission {:?}",
                perm
            );
            permissions.insert(*perm);
        }

        let final_permissions: Vec<Permission> = permissions.into_iter().collect();
        debug!(
            "resolve_user_permissions: final permissions for user_id={}: {:?}",
            user_id, final_permissions
        );
        Ok(final_permissions)
    }
}

