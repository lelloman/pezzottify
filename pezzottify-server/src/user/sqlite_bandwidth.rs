impl UserBandwidthStore for SqliteUserStore {
    fn record_bandwidth_usage(
        &self,
        user_id: usize,
        date: u32,
        endpoint_category: &str,
        bytes_sent: u64,
        request_count: u64,
    ) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Use INSERT OR REPLACE to upsert - if the unique constraint (user_id, date, endpoint_category) exists,
        // we need to add to existing values, so we use a subquery
        conn.execute(
            &format!(
                "INSERT INTO {} (user_id, date, endpoint_category, bytes_sent, request_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(user_id, date, endpoint_category) DO UPDATE SET
                 bytes_sent = bytes_sent + excluded.bytes_sent,
                 request_count = request_count + excluded.request_count,
                 updated = (cast(strftime('%s','now') as int))",
                BANDWIDTH_USAGE_TABLE_V_5.name
            ),
            params![
                user_id,
                date,
                endpoint_category,
                bytes_sent as i64,
                request_count as i64
            ],
        )?;

        record_db_query("record_bandwidth_usage", start.elapsed());
        Ok(())
    }

    fn get_user_bandwidth_usage(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT user_id, date, endpoint_category, bytes_sent, request_count
             FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY date DESC, endpoint_category",
            BANDWIDTH_USAGE_TABLE_V_5.name
        ))?;

        let records = stmt
            .query_map(params![user_id, start_date, end_date], |row| {
                Ok(BandwidthUsage {
                    user_id: row.get::<_, i64>(0)? as usize,
                    date: row.get::<_, i64>(1)? as u32,
                    endpoint_category: row.get(2)?,
                    bytes_sent: row.get::<_, i64>(3)? as u64,
                    request_count: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_bandwidth_usage", start.elapsed());
        Ok(records)
    }

    fn get_user_bandwidth_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        let records = self.get_user_bandwidth_usage(user_id, start_date, end_date)?;

        let mut summary = BandwidthSummary {
            user_id: Some(user_id),
            total_bytes_sent: 0,
            total_requests: 0,
            by_category: HashMap::new(),
        };

        for record in records {
            summary.total_bytes_sent += record.bytes_sent;
            summary.total_requests += record.request_count;

            let cat_entry = summary
                .by_category
                .entry(record.endpoint_category)
                .or_insert(CategoryBandwidth {
                    bytes_sent: 0,
                    request_count: 0,
                });
            cat_entry.bytes_sent += record.bytes_sent;
            cat_entry.request_count += record.request_count;
        }

        Ok(summary)
    }

    fn get_all_bandwidth_usage(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT user_id, date, endpoint_category, bytes_sent, request_count
             FROM {} WHERE date >= ?1 AND date <= ?2
             ORDER BY user_id, date DESC, endpoint_category",
            BANDWIDTH_USAGE_TABLE_V_5.name
        ))?;

        let records = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(BandwidthUsage {
                    user_id: row.get::<_, i64>(0)? as usize,
                    date: row.get::<_, i64>(1)? as u32,
                    endpoint_category: row.get(2)?,
                    bytes_sent: row.get::<_, i64>(3)? as u64,
                    request_count: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_all_bandwidth_usage", start.elapsed());
        Ok(records)
    }

    fn get_total_bandwidth_summary(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        let records = self.get_all_bandwidth_usage(start_date, end_date)?;

        let mut summary = BandwidthSummary {
            user_id: None,
            total_bytes_sent: 0,
            total_requests: 0,
            by_category: HashMap::new(),
        };

        for record in records {
            summary.total_bytes_sent += record.bytes_sent;
            summary.total_requests += record.request_count;

            let cat_entry = summary
                .by_category
                .entry(record.endpoint_category)
                .or_insert(CategoryBandwidth {
                    bytes_sent: 0,
                    request_count: 0,
                });
            cat_entry.bytes_sent += record.bytes_sent;
            cat_entry.request_count += record.request_count;
        }

        Ok(summary)
    }

    fn prune_bandwidth_usage(&self, older_than_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Calculate the cutoff date in YYYYMMDD format
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_secs = now - (older_than_days as u64 * 24 * 60 * 60);

        // Convert to YYYYMMDD format
        let cutoff_date = {
            let datetime = chrono::DateTime::from_timestamp(cutoff_secs as i64, 0)
                .unwrap_or_else(chrono::Utc::now);
            datetime
                .format("%Y%m%d")
                .to_string()
                .parse::<u32>()
                .unwrap_or(0)
        };

        let deleted = conn.execute(
            &format!(
                "DELETE FROM {} WHERE date < ?1",
                BANDWIDTH_USAGE_TABLE_V_5.name
            ),
            params![cutoff_date],
        )?;

        record_db_query("prune_bandwidth_usage", start.elapsed());
        Ok(deleted)
    }
}

