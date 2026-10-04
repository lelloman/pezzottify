impl user_store::PushRegistrationStore for SqliteUserStore {
    fn upsert_push_registration(
        &self,
        user_id: usize,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        device_id: Option<&str>,
    ) -> Result<()> {
        let start = Instant::now();
        let now = chrono::Utc::now().timestamp();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        // A re-registration (possibly by a different account on the same device)
        // replaces keys and owner but keeps the original creation time only when
        // the owner is unchanged.
        tx.execute(
            "INSERT INTO push_registrations
                (endpoint, user_id, p256dh, auth, device_id, created_at, updated_at,
                 last_success_at, first_failure_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL, NULL)
             ON CONFLICT(endpoint) DO UPDATE SET
                created_at = CASE WHEN user_id = excluded.user_id
                                  THEN created_at ELSE excluded.created_at END,
                user_id = excluded.user_id,
                p256dh = excluded.p256dh,
                auth = excluded.auth,
                device_id = excluded.device_id,
                updated_at = excluded.updated_at,
                first_failure_at = NULL",
            params![endpoint, user_id, p256dh, auth, device_id, now],
        )?;
        tx.execute(
            "DELETE FROM push_registrations
             WHERE user_id = ?1 AND endpoint NOT IN (
                 SELECT endpoint FROM push_registrations WHERE user_id = ?1
                 ORDER BY created_at DESC, updated_at DESC, endpoint DESC LIMIT ?2)",
            params![user_id, user_store::MAX_PUSH_REGISTRATIONS_PER_USER as i64],
        )?;
        tx.commit()?;
        record_db_query("upsert_push_registration", start.elapsed());
        Ok(())
    }

    fn delete_push_registration(&self, user_id: usize, endpoint: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM push_registrations WHERE user_id = ?1 AND endpoint = ?2",
            params![user_id, endpoint],
        )?;
        Ok(removed > 0)
    }

    fn list_push_registrations(&self, user_id: usize) -> Result<Vec<PushRegistration>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT user_id, endpoint, p256dh, auth, device_id, created_at, last_success_at,
                    first_failure_at
             FROM push_registrations WHERE user_id = ?1
             ORDER BY created_at, endpoint",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(PushRegistration {
                user_id: row.get::<_, i64>(0)? as usize,
                endpoint: row.get(1)?,
                p256dh: row.get(2)?,
                auth: row.get(3)?,
                device_id: row.get(4)?,
                created_at: row.get(5)?,
                last_success_at: row.get(6)?,
                first_failure_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn list_all_push_registrations(&self) -> Result<Vec<PushRegistrationOverview>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT p.user_id, p.endpoint, p.p256dh, p.auth, p.device_id, p.created_at,
                    p.last_success_at, p.first_failure_at, u.handle, d.id, d.device_name,
                    d.device_type
             FROM push_registrations p
             LEFT JOIN user u ON u.id = p.user_id
             LEFT JOIN device d ON d.device_uuid = p.device_id
             ORDER BY p.created_at DESC, p.endpoint",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PushRegistrationOverview {
                registration: PushRegistration {
                    user_id: row.get::<_, i64>(0)? as usize,
                    endpoint: row.get(1)?,
                    p256dh: row.get(2)?,
                    auth: row.get(3)?,
                    device_id: row.get(4)?,
                    created_at: row.get(5)?,
                    last_success_at: row.get(6)?,
                    first_failure_at: row.get(7)?,
                },
                user_handle: row.get(8)?,
                device_row_id: row.get::<_, Option<i64>>(9)?.map(|id| id as usize),
                device_name: row.get(10)?,
                device_type: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn remove_push_endpoint(&self, endpoint: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM push_registrations WHERE endpoint = ?1",
            params![endpoint],
        )?;
        Ok(removed > 0)
    }

    fn record_push_delivery(
        &self,
        endpoint: &str,
        delivered: bool,
        now: i64,
        max_failure_secs: i64,
    ) -> Result<PushDeliveryRecord> {
        let conn = self.conn.lock().unwrap();
        if delivered {
            conn.execute(
                "UPDATE push_registrations SET last_success_at = ?2, first_failure_at = NULL
                 WHERE endpoint = ?1",
                params![endpoint, now],
            )?;
            return Ok(PushDeliveryRecord::Kept);
        }
        conn.execute(
            "UPDATE push_registrations SET first_failure_at = COALESCE(first_failure_at, ?2)
             WHERE endpoint = ?1",
            params![endpoint, now],
        )?;
        let removed = conn.execute(
            "DELETE FROM push_registrations
             WHERE endpoint = ?1 AND first_failure_at IS NOT NULL AND first_failure_at <= ?2",
            params![endpoint, now - max_failure_secs],
        )?;
        Ok(if removed > 0 {
            PushDeliveryRecord::Removed
        } else {
            PushDeliveryRecord::Kept
        })
    }
}
