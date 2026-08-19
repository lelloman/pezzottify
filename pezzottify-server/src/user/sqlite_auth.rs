fn system_time_from_column_result(value: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(value as u64)
}

const AUTH_TOKEN_ABSOLUTE_TTL_SECS: i64 = 30 * 24 * 60 * 60;
const AUTH_TOKEN_IDLE_TTL_SECS: i64 = 7 * 24 * 60 * 60;
const AUTH_TOKEN_ID_HEX_LEN: usize = 12;

fn auth_token_digest(value: &AuthTokenValue) -> String {
    let digest = Sha256::digest(value.0.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn auth_token_identifier(digest: &str) -> &str {
    &digest[..AUTH_TOKEN_ID_HEX_LEN]
}

fn unix_timestamp_now() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("System clock is before the Unix epoch")?
        .as_secs() as i64)
}

fn auth_token_is_expired(created: i64, last_used: Option<i64>, now: i64) -> bool {
    let absolute_cutoff = now.saturating_sub(AUTH_TOKEN_ABSOLUTE_TTL_SECS);
    let idle_cutoff = now.saturating_sub(AUTH_TOKEN_IDLE_TTL_SECS);
    created <= absolute_cutoff || last_used.unwrap_or(created) <= idle_cutoff
}

fn delete_expired_auth_tokens(conn: &Connection, now: i64) -> Result<usize> {
    let absolute_cutoff = now.saturating_sub(AUTH_TOKEN_ABSOLUTE_TTL_SECS);
    let idle_cutoff = now.saturating_sub(AUTH_TOKEN_IDLE_TTL_SECS);
    Ok(conn.execute(
        "DELETE FROM auth_token
         WHERE created <= ?1 OR COALESCE(last_used, created) <= ?2",
        params![absolute_cutoff, idle_cutoff],
    )?)
}

impl UserAuthTokenStore for SqliteUserStore {
    fn get_user_auth_token(&self, value: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let digest = auth_token_digest(value);
        let row = conn
            .query_row(
                "SELECT user_id, created, last_used, device_id
                 FROM auth_token WHERE token_hash = ?1",
                params![digest],
                |row| {
                    Ok((
                        row.get::<_, usize>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<usize>>(3)?,
                    ))
                },
            )
            .optional()?;

        let result = match row {
            Some((user_id, created, last_used, device_id)) => {
                if auth_token_is_expired(created, last_used, unix_timestamp_now()?) {
                    conn.execute(
                        "DELETE FROM auth_token WHERE token_hash = ?1",
                        params![digest],
                    )?;
                    Ok(None)
                } else {
                    Ok(Some(AuthToken {
                        user_id,
                        device_id,
                        value: value.clone(),
                        created: system_time_from_column_result(created),
                        last_used: last_used.map(system_time_from_column_result),
                    }))
                }
            }
            None => Ok(None),
        };
        record_db_query("get_user_auth_token", start.elapsed());
        result
    }

    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Result<Option<AuthToken>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let digest = auth_token_digest(token);
        // Get the token data before deleting.
        let auth_token = match tx
            .prepare("SELECT user_id, created, last_used, device_id FROM auth_token WHERE token_hash = ?1")
            .and_then(|mut stmt| {
                stmt.query_row(params![digest], |row| {
                    Ok(AuthToken {
                        user_id: row.get(0)?,
                        device_id: row.get(3)?,
                        value: token.clone(),
                        created: system_time_from_column_result(row.get(1)?),
                        last_used: row
                            .get::<usize, Option<i64>>(2)?
                            .map(system_time_from_column_result),
                    })
                })
            }) {
                Ok(token) => token,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

        // Delete the token
        tx.execute(
            "DELETE FROM auth_token WHERE token_hash = ?1",
            params![digest],
        )?;

        tx.commit()?;
        Ok(Some(auth_token))
    }

    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = unix_timestamp_now()?;
        let digest = auth_token_digest(token);
        conn.execute(
            "UPDATE auth_token SET last_used = ?1 WHERE token_hash = ?2",
            params![now, digest],
        )?;
        Ok(())
    }

    fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let created = token
            .created
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let digest = auth_token_digest(&token.value);
        let token_id = auth_token_identifier(&digest);

        conn.execute(
            "INSERT INTO auth_token (user_id, token_hash, token_id, created, device_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token.user_id, digest, token_id, created, token.device_id],
        )?;
        record_db_query("add_user_auth_token", start.elapsed());
        Ok(())
    }

    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Result<Vec<AuthToken>> {
        let conn = self.conn.lock().unwrap();
        delete_expired_auth_tokens(&conn, unix_timestamp_now()?)?;
        let mut stmt = conn.prepare(
            "SELECT user_id, token_id, created, last_used, device_id
             FROM auth_token WHERE user_id = (SELECT id FROM user WHERE handle = ?1)",
        )?;
        let rows = stmt
            .query_map(params![user_handle], |row| {
                Ok(AuthToken {
                    user_id: row.get(0)?,
                    device_id: row.get(4)?,
                    value: AuthTokenValue(row.get(1)?),
                    created: system_time_from_column_result(row.get(2)?),
                    last_used: row
                        .get::<usize, Option<i64>>(3)?
                        .map(system_time_from_column_result),
                })
            })?
            .collect::<Result<Vec<AuthToken>, _>>()?;

        Ok(rows)
    }

    fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = unix_timestamp_now()?;
        let expired = delete_expired_auth_tokens(&conn, now)?;
        let cutoff_secs = now - (unused_for_days * 24 * 60 * 60) as i64;

        // Delete tokens that have never been used and are older than the cutoff
        // OR have been used but the last use is older than the cutoff
        let deleted = conn.execute(
            "DELETE FROM auth_token WHERE (last_used IS NULL AND created < ?1) OR (last_used IS NOT NULL AND last_used < ?1)",
            params![cutoff_secs],
        )?;

        Ok(expired + deleted)
    }
}

impl UserAuthCredentialsStore for SqliteUserStore {
    fn get_user_auth_credentials(&self, user_handle: &str) -> Result<Option<UserAuthCredentials>> {
        let start = Instant::now();
        let user_id = match self.get_user_id(user_handle)? {
            Some(id) => id,
            None => {
                record_db_query("get_user_auth_credentials", start.elapsed());
                return Ok(None);
            }
        };
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM user_password_credentials WHERE user_id = ?1")?;

        let password_credentials = match stmt.query_row(params![user_id], |row| {
            let hasher = match PezzottifyHasher::from_str(&row.get::<usize, String>(3)?) {
                Ok(x) => x,
                Err(_) => {
                    eprintln!("get_user_auth_credentials() -> Invalid hasher");
                    return Err(rusqlite::Error::InvalidQuery);
                }
            };
            let user_id: usize = row.get(0)?;
            let salt: String = row.get(1)?;
            let hash: String = row.get(2)?;
            let created = system_time_from_column_result(row.get(4).unwrap());
            Ok(UsernamePasswordCredentials {
                user_id,
                salt,
                hash,
                hasher,
                created,
                last_tried: row
                    .get::<usize, Option<i64>>(5)?
                    .map(system_time_from_column_result),
                last_used: row
                    .get::<usize, Option<i64>>(6)?
                    .map(system_time_from_column_result),
            })
        }) {
            Ok(creds) => Some(creds),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };

        record_db_query("get_user_auth_credentials", start.elapsed());
        Ok(Some(UserAuthCredentials {
            user_id,
            username_password: password_credentials,
            keys: vec![],
        }))
    }

    fn update_user_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let user_id = credentials.user_id;
        match credentials.username_password.as_ref() {
            Some(password_credentials) => {
                let updated = tx.execute(
                    "UPDATE user_password_credentials SET salt = ?1, hash = ?2, hasher = ?3 WHERE user_id = ?4",
                    params![
                        password_credentials.salt,
                        password_credentials.hash,
                        password_credentials.hasher.to_string(),
                        user_id
                    ],
                )?;
                if updated == 0 {
                    tx.execute(
                        "INSERT INTO user_password_credentials (salt, hash, hasher, user_id) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            password_credentials.salt,
                            password_credentials.hash,
                            password_credentials.hasher.to_string(),
                            user_id
                        ],
                    )?;
                }
            }
            None => {
                tx.execute(
                    "DELETE FROM user_password_credentials WHERE user_id = ?1",
                    params![user_id],
                )?;
            }
        };
        tx.commit()?;
        Ok(())
    }
}

