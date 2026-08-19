impl user_store::DeviceStore for SqliteUserStore {
    fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT ... ON CONFLICT for upsert semantics
        conn.execute(
            "INSERT INTO device (device_uuid, device_type, device_name, os_info, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(device_uuid) DO UPDATE SET
                device_type = ?2,
                device_name = ?3,
                os_info = ?4,
                last_seen = ?5",
            params![
                registration.device_uuid,
                registration.device_type.as_str(),
                registration.device_name,
                registration.os_info,
                now,
            ],
        )?;

        // Get the device ID (either newly created or existing)
        let device_id: usize = conn.query_row(
            "SELECT id FROM device WHERE device_uuid = ?1",
            params![registration.device_uuid],
            |row| row.get(0),
        )?;

        record_db_query("register_or_update_device", start.elapsed());
        Ok(device_id)
    }

    fn get_device(&self, device_id: usize) -> Result<Option<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE id = ?1",
            params![device_id],
            |row| {
                Ok(Device {
                    id: row.get(0)?,
                    device_uuid: row.get(1)?,
                    user_id: row.get(2)?,
                    device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                    device_name: row.get(4)?,
                    os_info: row.get(5)?,
                    first_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        record_db_query("get_device", start.elapsed());
        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_device_by_uuid(&self, device_uuid: &str) -> Result<Option<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE device_uuid = ?1",
            params![device_uuid],
            |row| {
                Ok(Device {
                    id: row.get(0)?,
                    device_uuid: row.get(1)?,
                    user_id: row.get(2)?,
                    device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                    device_name: row.get(4)?,
                    os_info: row.get(5)?,
                    first_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        record_db_query("get_device_by_uuid", start.elapsed());
        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE user_id = ?1 ORDER BY last_seen DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(Device {
                id: row.get(0)?,
                device_uuid: row.get(1)?,
                user_id: row.get(2)?,
                device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                device_name: row.get(4)?,
                os_info: row.get(5)?,
                first_seen: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                last_seen: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
            })
        })?;

        let devices: Result<Vec<Device>, _> = rows.collect();
        record_db_query("get_user_devices", start.elapsed());
        Ok(devices?)
    }

    fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE device SET user_id = ?1 WHERE id = ?2",
            params![user_id, device_id],
        )?;
        record_db_query("associate_device_with_user", start.elapsed());
        Ok(())
    }

    fn touch_device(&self, device_id: usize) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE device SET last_seen = ?1 WHERE id = ?2",
            params![now, device_id],
        )?;
        record_db_query("touch_device", start.elapsed());
        Ok(())
    }

    fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - (inactive_for_days as i64 * 24 * 60 * 60);

        let deleted = conn.execute(
            "DELETE FROM device WHERE user_id IS NULL AND last_seen < ?1",
            params![cutoff],
        )?;
        record_db_query("prune_orphaned_devices", start.elapsed());
        Ok(deleted)
    }

    fn prune_inactive_devices(&self, inactive_for_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - (inactive_for_days as i64 * 24 * 60 * 60);

        let deleted = conn.execute("DELETE FROM device WHERE last_seen < ?1", params![cutoff])?;
        record_db_query("prune_inactive_devices", start.elapsed());
        Ok(deleted)
    }

    fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Count current devices for user
        let device_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM device WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )?;

        if device_count <= max_devices {
            record_db_query("enforce_user_device_limit", start.elapsed());
            return Ok(0);
        }

        let to_delete = device_count - max_devices;

        // Delete oldest devices (by last_seen) beyond the limit
        let deleted = conn.execute(
            "DELETE FROM device WHERE id IN (
                SELECT id FROM device WHERE user_id = ?1
                ORDER BY last_seen ASC LIMIT ?2
            )",
            params![user_id, to_delete],
        )?;

        record_db_query("enforce_user_device_limit", start.elapsed());
        Ok(deleted)
    }

    fn get_device_share_policy(&self, device_id: usize) -> Result<DeviceSharePolicy> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let policy_row: Option<(String,)> = conn
            .query_row(
                "SELECT mode FROM device_share_policy WHERE device_id = ?1",
                params![device_id],
                |row| Ok((row.get(0)?,)),
            )
            .optional()?;

        let mode = match policy_row.map(|(m,)| m) {
            Some(m) => match m.as_str() {
                "allow_everyone" => DeviceShareMode::AllowEveryone,
                "deny_everyone" => DeviceShareMode::DenyEveryone,
                "custom" => DeviceShareMode::Custom,
                _ => DeviceShareMode::DenyEveryone,
            },
            None => {
                record_db_query("get_device_share_policy", start.elapsed());
                return Ok(DeviceSharePolicy::default());
            }
        };

        let mut allow_users = Vec::new();
        let mut allow_roles = Vec::new();
        let mut deny_users = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT rule_type, subject_type, subject_value
             FROM device_share_rule WHERE device_id = ?1",
        )?;
        let rows = stmt.query_map(params![device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (rule_type, subject_type, subject_value) = row?;
            match (rule_type.as_str(), subject_type.as_str()) {
                ("allow", "user_id") => {
                    if let Ok(id) = subject_value.parse::<usize>() {
                        allow_users.push(id);
                    }
                }
                ("allow", "role") => {
                    if let Some(role) = UserRole::from_str(&subject_value) {
                        allow_roles.push(role);
                    }
                }
                ("deny", "user_id") => {
                    if let Ok(id) = subject_value.parse::<usize>() {
                        deny_users.push(id);
                    }
                }
                _ => {}
            }
        }

        record_db_query("get_device_share_policy", start.elapsed());
        Ok(DeviceSharePolicy {
            mode,
            allow_users,
            allow_roles,
            deny_users,
        })
    }

    fn set_device_share_policy(&self, device_id: usize, policy: &DeviceSharePolicy) -> Result<()> {
        policy.validate()?;

        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mode_str = match policy.mode {
            DeviceShareMode::AllowEveryone => "allow_everyone",
            DeviceShareMode::DenyEveryone => "deny_everyone",
            DeviceShareMode::Custom => "custom",
        };

        conn.execute(
            "INSERT INTO device_share_policy (device_id, mode, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(device_id) DO UPDATE SET
                mode = ?2,
                updated_at = ?3",
            params![device_id, mode_str, now],
        )?;

        conn.execute(
            "DELETE FROM device_share_rule WHERE device_id = ?1",
            params![device_id],
        )?;

        if policy.mode == DeviceShareMode::Custom {
            for user_id in &policy.allow_users {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'allow', 'user_id', ?2, ?3)",
                    params![device_id, user_id.to_string(), now],
                )?;
            }
            for role in &policy.allow_roles {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'allow', 'role', ?2, ?3)",
                    params![device_id, role.as_str().to_lowercase(), now],
                )?;
            }
            for user_id in &policy.deny_users {
                conn.execute(
                    "INSERT OR IGNORE INTO device_share_rule (device_id, rule_type, subject_type, subject_value, created_at)
                     VALUES (?1, 'deny', 'user_id', ?2, ?3)",
                    params![device_id, user_id.to_string(), now],
                )?;
            }
        }

        record_db_query("set_device_share_policy", start.elapsed());
        Ok(())
    }
}

