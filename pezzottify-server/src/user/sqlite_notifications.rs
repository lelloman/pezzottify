impl crate::notifications::NotificationStore for SqliteUserStore {
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: crate::notifications::NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<crate::notifications::Notification> {
        let start = Instant::now();
        let id = format!("notif_{}", random_string(16));
        let created_at = chrono::Utc::now().timestamp();
        let type_str = serde_json::to_string(&notification_type)?;
        let data_str = serde_json::to_string(&data)?;

        let conn = self.conn.lock().unwrap();

        // Insert the notification
        conn.execute(
            "INSERT INTO user_notifications (id, user_id, notification_type, title, body, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, user_id, type_str, title, body, data_str, created_at],
        )?;

        // Enforce 100-per-user limit: delete oldest beyond limit
        // Use rowid as tiebreaker when timestamps are equal (e.g., rapid inserts)
        conn.execute(
            "DELETE FROM user_notifications WHERE user_id = ?1 AND id NOT IN (
                SELECT id FROM user_notifications WHERE user_id = ?1
                ORDER BY created_at DESC, rowid DESC LIMIT 100
            )",
            params![user_id],
        )?;

        record_db_query("create_notification", start.elapsed());

        Ok(crate::notifications::Notification {
            id,
            notification_type,
            title,
            body,
            data,
            read_at: None,
            created_at,
        })
    }

    fn get_user_notifications(
        &self,
        user_id: usize,
    ) -> Result<Vec<crate::notifications::Notification>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, notification_type, title, body, data, read_at, created_at
             FROM user_notifications
             WHERE user_id = ?1
             ORDER BY created_at DESC, rowid DESC",
        )?;

        let notifications = stmt
            .query_map(params![user_id], |row| {
                let id: String = row.get(0)?;
                let type_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let body: Option<String> = row.get(3)?;
                let data_str: String = row.get(4)?;
                let read_at: Option<i64> = row.get(5)?;
                let created_at: i64 = row.get(6)?;

                Ok((id, type_str, title, body, data_str, read_at, created_at))
            })?
            .filter_map(|r| r.ok())
            .filter_map(
                |(id, type_str, title, body, data_str, read_at, created_at)| {
                    let notification_type: crate::notifications::NotificationType =
                        serde_json::from_str(&type_str).ok()?;
                    let data: serde_json::Value = serde_json::from_str(&data_str).ok()?;

                    Some(crate::notifications::Notification {
                        id,
                        notification_type,
                        title,
                        body,
                        data,
                        read_at,
                        created_at,
                    })
                },
            )
            .collect();

        record_db_query("get_user_notifications", start.elapsed());
        Ok(notifications)
    }

    fn get_notification(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, notification_type, title, body, data, read_at, created_at
                 FROM user_notifications
                 WHERE id = ?1 AND user_id = ?2",
                params![notification_id, user_id],
                |row| {
                    let id: String = row.get(0)?;
                    let type_str: String = row.get(1)?;
                    let title: String = row.get(2)?;
                    let body: Option<String> = row.get(3)?;
                    let data_str: String = row.get(4)?;
                    let read_at: Option<i64> = row.get(5)?;
                    let created_at: i64 = row.get(6)?;
                    Ok((id, type_str, title, body, data_str, read_at, created_at))
                },
            )
            .optional()?;

        record_db_query("get_notification", start.elapsed());

        match result {
            Some((id, type_str, title, body, data_str, read_at, created_at)) => {
                let notification_type: crate::notifications::NotificationType =
                    serde_json::from_str(&type_str)?;
                let data: serde_json::Value = serde_json::from_str(&data_str)?;

                Ok(Some(crate::notifications::Notification {
                    id,
                    notification_type,
                    title,
                    body,
                    data,
                    read_at,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    fn mark_notification_read(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        let start = Instant::now();
        let read_at = chrono::Utc::now().timestamp();

        let conn = self.conn.lock().unwrap();

        let rows_affected = conn.execute(
            "UPDATE user_notifications SET read_at = ?1 WHERE id = ?2 AND user_id = ?3 AND read_at IS NULL",
            params![read_at, notification_id, user_id],
        )?;

        record_db_query("mark_notification_read", start.elapsed());

        if rows_affected == 0 {
            // Either doesn't exist, doesn't belong to user, or already read
            // Try to fetch it to check if it exists and belongs to user
            drop(conn);
            return self.get_notification(notification_id, user_id);
        }

        // Fetch and return the updated notification
        drop(conn);
        self.get_notification(notification_id, user_id)
    }

    fn get_unread_count(&self, user_id: usize) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = ?1 AND read_at IS NULL",
            params![user_id],
            |row| row.get(0),
        )?;

        record_db_query("get_unread_count", start.elapsed());
        Ok(count as usize)
    }
}

include!("sqlite_user_store_tests.rs");
