impl UserSettingsStore for SqliteUserStore {
    fn get_user_setting(&self, user_id: usize, key: &str) -> Result<Option<UserSetting>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT setting_value FROM user_settings WHERE user_id = ?1 AND setting_key = ?2",
            params![user_id, key],
            |row| row.get::<usize, Option<String>>(0),
        );

        record_db_query("get_user_setting", start.elapsed());

        match result {
            Ok(Some(value)) => {
                let setting =
                    UserSetting::from_key_value(key, &value).map_err(|e| anyhow::anyhow!(e))?;
                Ok(Some(setting))
            }
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_user_setting(&self, user_id: usize, setting: UserSetting) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let key = setting.key();
        let value = setting.value_to_string();

        conn.execute(
            "INSERT INTO user_settings (user_id, setting_key, setting_value, updated)
             VALUES (?1, ?2, ?3, (cast(strftime('%s','now') as int)))
             ON CONFLICT(user_id, setting_key) DO UPDATE SET
                 setting_value = excluded.setting_value,
                 updated = excluded.updated",
            params![user_id, key, value],
        )?;

        record_db_query("set_user_setting", start.elapsed());
        Ok(())
    }

    fn get_all_user_settings(&self, user_id: usize) -> Result<Vec<UserSetting>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT setting_key, setting_value FROM user_settings WHERE user_id = ?1")?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            let (key, value) = row?;
            // Skip unknown keys for forward compatibility
            if let Ok(setting) = UserSetting::from_key_value(&key, &value) {
                settings.push(setting);
            }
        }

        record_db_query("get_all_user_settings", start.elapsed());
        Ok(settings)
    }

    fn get_user_ids_with_setting(&self, key: &str, value: &str) -> Result<Vec<usize>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT user_id FROM user_settings WHERE setting_key = ?1 AND setting_value = ?2",
        )?;
        let rows = stmt.query_map(params![key, value], |row| row.get::<usize, usize>(0))?;

        let mut user_ids = Vec::new();
        for row in rows {
            user_ids.push(row?);
        }

        record_db_query("get_user_ids_with_setting", start.elapsed());
        Ok(user_ids)
    }
}

