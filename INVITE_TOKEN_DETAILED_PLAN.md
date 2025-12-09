# Invite Token Feature - Detailed Implementation Plan

This document breaks down the Invite Token implementation into small, sequential, actionable tasks.

---

## Overview

**Feature**: One-time login links for existing users. Admin creates a time-limited token tied to a user, shares the link, recipient clicks and gets logged in automatically.

**Files to modify**:
- `catalog-server/src/user/sqlite_user_store.rs` - Table definition, trait impl
- `catalog-server/src/user/user_store.rs` - Trait method signatures
- `catalog-server/src/user/user_manager.rs` - High-level API
- `catalog-server/src/user/auth.rs` - `InviteToken` type
- `catalog-server/src/user/mod.rs` - Export new types
- `catalog-server/src/server/server.rs` - Route handlers, request/response types

---

## Phase 1: Database Layer

### Task 1.1: Define InviteToken table schema
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Add a new table definition constant for the `invite_token` table.
- **Context**: Follow the existing pattern for table definitions (e.g., `AUTH_TOKEN_TABLE_V_0`, `DEVICE_TABLE_V_8`). Add after the last table definition constant (around line 588).

```rust
const INVITE_TOKEN_TABLE_V_9: Table = Table {
    name: "invite_token",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!("token", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "created_by",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "created",
            &SqlType::Integer,
            default_value = Some(DEFAULT_TIMESTAMP)
        ),
        sqlite_column!("expires", &SqlType::Integer, non_null = true),
        sqlite_column!("used_at", &SqlType::Integer),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_invite_token_token", "token"),
        ("idx_invite_token_user_id", "user_id"),
    ],
};
```

---

### Task 1.2: Add new schema version to VERSIONED_SCHEMAS
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Add a new `VersionedSchema` entry (V9) that includes the `invite_token` table with a migration function.
- **Context**: Add after the last entry in `VERSIONED_SCHEMAS` array (around line 790). The migration creates the new table.

```rust
VersionedSchema {
    version: 9,
    tables: &[
        USER_TABLE_V_0,
        LIKED_CONTENT_TABLE_V_2,
        AUTH_TOKEN_TABLE_V_8,
        USER_PASSWORD_CREDENTIALS_V_0,
        USER_PLAYLIST_TABLE_V_3,
        USER_PLAYLIST_TRACKS_TABLE_V_3,
        USER_ROLE_TABLE_V_4,
        USER_EXTRA_PERMISSION_V_4,
        BANDWIDTH_USAGE_TABLE_V_5,
        LISTENING_EVENT_TABLE_V_6,
        USER_SETTINGS_TABLE_V_7,
        DEVICE_TABLE_V_8,
        USER_EVENTS_TABLE_V_8,
        INVITE_TOKEN_TABLE_V_9,
    ],
    migration: Some(|conn: &Connection| {
        conn.execute(
            "CREATE TABLE invite_token (
                id INTEGER PRIMARY KEY,
                token TEXT NOT NULL UNIQUE,
                user_id INTEGER NOT NULL,
                created_by INTEGER NOT NULL,
                created INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                expires INTEGER NOT NULL,
                used_at INTEGER,
                FOREIGN KEY (user_id) REFERENCES user(id) ON DELETE CASCADE,
                FOREIGN KEY (created_by) REFERENCES user(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX idx_invite_token_token ON invite_token(token)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX idx_invite_token_user_id ON invite_token(user_id)",
            [],
        )?;
        Ok(())
    }),
},
```

---

## Phase 2: Data Types

### Task 2.1: Define InviteToken struct in auth.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/auth.rs`
- **Description**: Add the `InviteToken` struct with helper methods.
- **Context**: Add at the end of the file, before the `#[cfg(test)]` module.

```rust
#[derive(Debug, Clone)]
pub struct InviteToken {
    pub id: usize,
    pub token: String,
    pub user_id: usize,
    pub created_by: usize,
    pub created: SystemTime,
    pub expires: SystemTime,
    pub used_at: Option<SystemTime>,
}

impl InviteToken {
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires
    }

    pub fn is_used(&self) -> bool {
        self.used_at.is_some()
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_used()
    }
}
```

---

### Task 2.2: Define InviteTokenView struct for API responses
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/auth.rs`
- **Description**: Add a serializable view struct for API responses that includes user handles.
- **Context**: Add immediately after the `InviteToken` struct.

```rust
/// For API responses - includes user handles for display
#[derive(Debug, Clone, Serialize)]
pub struct InviteTokenView {
    pub id: usize,
    pub token: String,
    pub user_id: usize,
    pub user_handle: String,
    pub created_by: usize,
    pub created_by_handle: String,
    pub created: u64,
    pub expires: u64,
    pub used_at: Option<u64>,
}
```

---

### Task 2.3: Export new types from mod.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/mod.rs`
- **Description**: Export `InviteToken` and `InviteTokenView` from the auth module.
- **Context**: Update the `pub use auth::` line (line 13) to include the new types.

```rust
pub use auth::{AuthToken, AuthTokenValue, InviteToken, InviteTokenView, UserAuthCredentials, UsernamePasswordCredentials};
```

---

## Phase 3: Store Trait

### Task 3.1: Add InviteTokenStore trait to user_store.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_store.rs`
- **Description**: Define a new trait `InviteTokenStore` with all invite token operations.
- **Context**: Add after the `UserEventStore` trait (around line 331), before `FullUserStore`.

```rust
use super::auth::InviteToken;

/// Trait for invite token storage operations
pub trait InviteTokenStore: Send + Sync {
    /// Creates a new invite token for a user.
    /// Returns the token id.
    fn create_invite_token(
        &self,
        token: &str,
        user_id: usize,
        created_by: usize,
        expires: SystemTime,
    ) -> Result<usize>;

    /// Gets an invite token by its id.
    fn get_invite_token_by_id(&self, token_id: usize) -> Result<Option<InviteToken>>;

    /// Gets an invite token by its token value.
    fn get_invite_token_by_value(&self, token: &str) -> Result<Option<InviteToken>>;

    /// Marks an invite token as used (sets used_at to current time).
    fn mark_invite_token_used(&self, token_id: usize) -> Result<()>;

    /// Deletes an invite token by its id.
    fn delete_invite_token(&self, token_id: usize) -> Result<()>;

    /// Lists all invite tokens.
    fn list_all_invite_tokens(&self) -> Result<Vec<InviteToken>>;

    /// Lists invite tokens for a specific user.
    fn list_invite_tokens_for_user(&self, user_id: usize) -> Result<Vec<InviteToken>>;
}
```

---

### Task 3.2: Add InviteTokenStore to FullUserStore trait
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_store.rs`
- **Description**: Include `InviteTokenStore` in the `FullUserStore` trait bound and blanket implementation.
- **Context**: Update the trait definition (around line 334) and the blanket impl (around line 345).

```rust
/// Combined trait for user storage with all sub-stores
pub trait FullUserStore:
    UserStore
    + UserBandwidthStore
    + UserListeningStore
    + UserSettingsStore
    + DeviceStore
    + UserEventStore
    + InviteTokenStore
{
}

// Blanket implementation
impl<
        T: UserStore
            + UserBandwidthStore
            + UserListeningStore
            + UserSettingsStore
            + DeviceStore
            + UserEventStore
            + InviteTokenStore,
    > FullUserStore for T
{
}
```

---

### Task 3.3: Add SystemTime import to user_store.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_store.rs`
- **Description**: Add `SystemTime` import if not already present.
- **Context**: Add to imports at the top of the file.

```rust
use std::time::SystemTime;
```

---

## Phase 4: SQLite Store Implementation

### Task 4.1: Add token generation helper function
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Add a helper function to generate secure random invite token values.
- **Context**: Add near the existing `generate_random_id` function (around line 810).

```rust
fn generate_invite_token_value() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}
```

Note: Need to add `getrandom` and `base64` dependencies to Cargo.toml if not present.

---

### Task 4.2: Implement InviteTokenStore for SqliteUserStore - create_invite_token
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement the `create_invite_token` method.
- **Context**: Add a new `impl InviteTokenStore for SqliteUserStore` block after the existing trait implementations.

```rust
impl InviteTokenStore for SqliteUserStore {
    fn create_invite_token(
        &self,
        token: &str,
        user_id: usize,
        created_by: usize,
        expires: SystemTime,
    ) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let expires_ts = expires
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO invite_token (token, user_id, created_by, expires) VALUES (?1, ?2, ?3, ?4)",
            params![token, user_id, created_by, expires_ts],
        )?;

        let id = conn.last_insert_rowid() as usize;
        record_db_query("create_invite_token", start.elapsed());
        Ok(id)
    }
    // ... other methods follow
}
```

---

### Task 4.3: Implement InviteTokenStore - get_invite_token_by_id
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement fetching an invite token by its ID.

```rust
fn get_invite_token_by_id(&self, token_id: usize) -> Result<Option<InviteToken>> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();
    let result = conn
        .query_row(
            "SELECT id, token, user_id, created_by, created, expires, used_at
             FROM invite_token WHERE id = ?1",
            params![token_id],
            |row| {
                Ok(InviteToken {
                    id: row.get::<_, i64>(0)? as usize,
                    token: row.get(1)?,
                    user_id: row.get::<_, i64>(2)? as usize,
                    created_by: row.get::<_, i64>(3)? as usize,
                    created: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(4)? as u64),
                    expires: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(5)? as u64),
                    used_at: row.get::<_, Option<i64>>(6)?.map(|ts| {
                        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64)
                    }),
                })
            },
        )
        .optional()?;

    record_db_query("get_invite_token_by_id", start.elapsed());
    Ok(result)
}
```

---

### Task 4.4: Implement InviteTokenStore - get_invite_token_by_value
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement fetching an invite token by its token string value.

```rust
fn get_invite_token_by_value(&self, token: &str) -> Result<Option<InviteToken>> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();
    let result = conn
        .query_row(
            "SELECT id, token, user_id, created_by, created, expires, used_at
             FROM invite_token WHERE token = ?1",
            params![token],
            |row| {
                Ok(InviteToken {
                    id: row.get::<_, i64>(0)? as usize,
                    token: row.get(1)?,
                    user_id: row.get::<_, i64>(2)? as usize,
                    created_by: row.get::<_, i64>(3)? as usize,
                    created: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(4)? as u64),
                    expires: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(row.get::<_, i64>(5)? as u64),
                    used_at: row.get::<_, Option<i64>>(6)?.map(|ts| {
                        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64)
                    }),
                })
            },
        )
        .optional()?;

    record_db_query("get_invite_token_by_value", start.elapsed());
    Ok(result)
}
```

---

### Task 4.5: Implement InviteTokenStore - mark_invite_token_used
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement marking a token as used.

```rust
fn mark_invite_token_used(&self, token_id: usize) -> Result<()> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let rows = conn.execute(
        "UPDATE invite_token SET used_at = ?1 WHERE id = ?2",
        params![now, token_id],
    )?;

    record_db_query("mark_invite_token_used", start.elapsed());

    if rows == 0 {
        bail!("Invite token with id {} not found", token_id);
    }
    Ok(())
}
```

---

### Task 4.6: Implement InviteTokenStore - delete_invite_token
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement deleting a token by ID.

```rust
fn delete_invite_token(&self, token_id: usize) -> Result<()> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();

    let rows = conn.execute(
        "DELETE FROM invite_token WHERE id = ?1",
        params![token_id],
    )?;

    record_db_query("delete_invite_token", start.elapsed());

    if rows == 0 {
        bail!("Invite token with id {} not found", token_id);
    }
    Ok(())
}
```

---

### Task 4.7: Implement InviteTokenStore - list_all_invite_tokens
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement listing all invite tokens.

```rust
fn list_all_invite_tokens(&self) -> Result<Vec<InviteToken>> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, token, user_id, created_by, created, expires, used_at
         FROM invite_token ORDER BY created DESC",
    )?;

    let tokens = stmt
        .query_map([], |row| {
            Ok(InviteToken {
                id: row.get::<_, i64>(0)? as usize,
                token: row.get(1)?,
                user_id: row.get::<_, i64>(2)? as usize,
                created_by: row.get::<_, i64>(3)? as usize,
                created: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(4)? as u64),
                expires: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(5)? as u64),
                used_at: row.get::<_, Option<i64>>(6)?.map(|ts| {
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64)
                }),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    record_db_query("list_all_invite_tokens", start.elapsed());
    Ok(tokens)
}
```

---

### Task 4.8: Implement InviteTokenStore - list_invite_tokens_for_user
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Implement listing invite tokens for a specific user.

```rust
fn list_invite_tokens_for_user(&self, user_id: usize) -> Result<Vec<InviteToken>> {
    let start = Instant::now();
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, token, user_id, created_by, created, expires, used_at
         FROM invite_token WHERE user_id = ?1 ORDER BY created DESC",
    )?;

    let tokens = stmt
        .query_map(params![user_id], |row| {
            Ok(InviteToken {
                id: row.get::<_, i64>(0)? as usize,
                token: row.get(1)?,
                user_id: row.get::<_, i64>(2)? as usize,
                created_by: row.get::<_, i64>(3)? as usize,
                created: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(4)? as u64),
                expires: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(row.get::<_, i64>(5)? as u64),
                used_at: row.get::<_, Option<i64>>(6)?.map(|ts| {
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64)
                }),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    record_db_query("list_invite_tokens_for_user", start.elapsed());
    Ok(tokens)
}
```

---

### Task 4.9: Add InviteTokenStore import to sqlite_user_store.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Import the `InviteTokenStore` trait.
- **Context**: Update the imports from `user_store` (around line 14).

```rust
use crate::user::user_store::{InviteTokenStore, UserBandwidthStore, UserListeningStore, UserSettingsStore};
```

---

### Task 4.10: Add dependencies to Cargo.toml (if needed)
- [ ] **Status**: Not started
- **File**: `catalog-server/Cargo.toml`
- **Description**: Add `getrandom` and verify `base64` dependencies are present for token generation.
- **Context**: Check if these are already in dependencies, add if missing.

```toml
getrandom = "0.2"
base64 = "0.21"  # or whatever version is being used
```

---

## Phase 5: UserManager Methods

### Task 5.1: Add create_invite_token method to UserManager
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add high-level method to create an invite token.
- **Context**: Add in the `impl UserManager` block, in a new section for invite tokens.

```rust
// ========================================================================
// Invite Token Methods
// ========================================================================

const DEFAULT_INVITE_TOKEN_EXPIRY_HOURS: u64 = 4;

/// Creates an invite token for a user.
/// Returns the created InviteToken with the generated token value.
pub fn create_invite_token(
    &self,
    user_handle: &str,
    created_by_id: usize,
    expires_in_hours: Option<u64>,
) -> Result<super::InviteToken> {
    let user_id = self
        .user_store
        .get_user_id(user_handle)?
        .with_context(|| format!("User with handle '{}' not found", user_handle))?;

    let expires_hours = expires_in_hours.unwrap_or(DEFAULT_INVITE_TOKEN_EXPIRY_HOURS);
    let expires = SystemTime::now() + std::time::Duration::from_secs(expires_hours * 3600);

    let token_value = generate_invite_token_value();

    let token_id = self.user_store.create_invite_token(
        &token_value,
        user_id,
        created_by_id,
        expires,
    )?;

    Ok(super::InviteToken {
        id: token_id,
        token: token_value,
        user_id,
        created_by: created_by_id,
        created: SystemTime::now(),
        expires,
        used_at: None,
    })
}
```

Also add the token generation helper at module level:

```rust
fn generate_invite_token_value() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}
```

---

### Task 5.2: Add redeem_invite_token method to UserManager
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add method to redeem an invite token and log the user in.

```rust
/// Redeems an invite token and returns an auth token for the user.
/// This is the public login flow for invite tokens.
pub fn redeem_invite_token(
    &mut self,
    token_value: &str,
    device_registration: &super::device::DeviceRegistration,
) -> Result<(super::AuthToken, usize)> {
    // 1. Look up token
    let invite_token = self
        .user_store
        .get_invite_token_by_value(token_value)?
        .with_context(|| "Invalid invite token")?;

    // 2. Validate token
    if invite_token.is_used() {
        bail!("Invite token has already been used");
    }
    if invite_token.is_expired() {
        bail!("Invite token has expired");
    }

    // 3. Register/update device
    let device_id = self.user_store.register_or_update_device(device_registration)?;
    self.user_store.associate_device_with_user(device_id, invite_token.user_id)?;

    // 4. Get user credentials to generate auth token
    let user_handle = self
        .user_store
        .get_user_handle(invite_token.user_id)?
        .with_context(|| "User not found")?;

    let credentials = self
        .user_store
        .get_user_auth_credentials(&user_handle)?
        .unwrap_or_else(|| super::UserAuthCredentials {
            user_id: invite_token.user_id,
            username_password: None,
            keys: vec![],
        });

    // 5. Generate auth token
    let auth_token = self.generate_auth_token(&credentials, device_id)?;

    // 6. Mark invite token as used
    self.user_store.mark_invite_token_used(invite_token.id)?;

    Ok((auth_token, invite_token.user_id))
}
```

---

### Task 5.3: Add revoke_invite_token method to UserManager
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add method to revoke/delete an invite token.

```rust
/// Revokes (deletes) an invite token by ID.
pub fn revoke_invite_token(&self, token_id: usize) -> Result<()> {
    self.user_store.delete_invite_token(token_id)
}
```

---

### Task 5.4: Add list methods for invite tokens to UserManager
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add methods to list invite tokens with user handle resolution.

```rust
/// Lists all invite tokens with resolved user handles.
pub fn list_all_invite_tokens(&self) -> Result<Vec<super::auth::InviteTokenView>> {
    let tokens = self.user_store.list_all_invite_tokens()?;
    self.tokens_to_views(tokens)
}

/// Lists invite tokens for a specific user.
pub fn list_invite_tokens_for_user(&self, user_handle: &str) -> Result<Vec<super::auth::InviteTokenView>> {
    let user_id = self
        .user_store
        .get_user_id(user_handle)?
        .with_context(|| format!("User with handle '{}' not found", user_handle))?;

    let tokens = self.user_store.list_invite_tokens_for_user(user_id)?;
    self.tokens_to_views(tokens)
}

/// Gets a single invite token by ID with resolved handles.
pub fn get_invite_token(&self, token_id: usize) -> Result<Option<super::auth::InviteTokenView>> {
    let token = self.user_store.get_invite_token_by_id(token_id)?;
    match token {
        Some(t) => Ok(Some(self.token_to_view(t)?)),
        None => Ok(None),
    }
}

// Helper to convert tokens to views with resolved handles
fn tokens_to_views(&self, tokens: Vec<super::InviteToken>) -> Result<Vec<super::auth::InviteTokenView>> {
    tokens.into_iter().map(|t| self.token_to_view(t)).collect()
}

fn token_to_view(&self, token: super::InviteToken) -> Result<super::auth::InviteTokenView> {
    let user_handle = self
        .user_store
        .get_user_handle(token.user_id)?
        .unwrap_or_else(|| format!("deleted-{}", token.user_id));

    let created_by_handle = self
        .user_store
        .get_user_handle(token.created_by)?
        .unwrap_or_else(|| format!("deleted-{}", token.created_by));

    Ok(super::auth::InviteTokenView {
        id: token.id,
        token: token.token,
        user_id: token.user_id,
        user_handle,
        created_by: token.created_by,
        created_by_handle,
        created: token.created
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        expires: token.expires
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        used_at: token.used_at.map(|t| {
            t.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }),
    })
}
```

---

### Task 5.5: Add imports to user_manager.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add necessary imports for invite token functionality.
- **Context**: Update the imports at the top of the file.

```rust
// Add to existing super:: imports
use super::auth::InviteTokenView;
```

---

## Phase 6: API Endpoints

### Task 6.1: Add request/response types for invite token endpoints
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add request and response structs for the invite token API.
- **Context**: Add near other request/response types in the file.

```rust
#[derive(Deserialize)]
struct CreateInviteTokenBody {
    expires_in_hours: Option<u64>,
}

#[derive(Serialize)]
struct CreateInviteTokenResponse {
    id: usize,
    token: String,
    user_handle: String,
    expires_at: u64,
    link: String,
}

#[derive(Deserialize)]
struct RedeemInviteBody {
    token: String,
    device_uuid: String,
    device_type: String,
    device_name: String,
    os_info: Option<String>,
}

#[derive(Serialize)]
struct InviteTokenResponse {
    id: usize,
    token: String,
    user_id: usize,
    user_handle: String,
    created_by: usize,
    created_by_handle: String,
    created: u64,
    expires: u64,
    used_at: Option<u64>,
}
```

---

### Task 6.2: Add POST /v1/admin/users/{handle}/invite-token endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add endpoint to create an invite token for a user.
- **Context**: Add to the admin routes section.

```rust
async fn create_invite_token_for_user(
    State(state): State<AppState>,
    Path(user_handle): Path<String>,
    session: Session,
    Json(body): Json<CreateInviteTokenBody>,
    headers: HeaderMap,
) -> Result<Json<CreateInviteTokenResponse>, AppError> {
    require_manage_permissions(&session)?;

    let mut user_manager = state.user_manager.write().await;
    let token = user_manager.create_invite_token(
        &user_handle,
        session.user_id,
        body.expires_in_hours,
    )?;

    // Build link from Host header
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = if host.contains("localhost") { "http" } else { "https" };
    let link = format!(
        "pezzottify://invite?server={}://{}&token={}",
        scheme, host, token.token
    );

    Ok(Json(CreateInviteTokenResponse {
        id: token.id,
        token: token.token,
        user_handle,
        expires_at: token.expires
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        link,
    }))
}
```

---

### Task 6.3: Add GET /v1/admin/invite-tokens endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add endpoint to list all invite tokens.

```rust
async fn list_all_invite_tokens(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<InviteTokenResponse>>, AppError> {
    require_manage_permissions(&session)?;

    let user_manager = state.user_manager.read().await;
    let tokens = user_manager.list_all_invite_tokens()?;

    let response: Vec<InviteTokenResponse> = tokens
        .into_iter()
        .map(|t| InviteTokenResponse {
            id: t.id,
            token: t.token,
            user_id: t.user_id,
            user_handle: t.user_handle,
            created_by: t.created_by,
            created_by_handle: t.created_by_handle,
            created: t.created,
            expires: t.expires,
            used_at: t.used_at,
        })
        .collect();

    Ok(Json(response))
}
```

---

### Task 6.4: Add GET /v1/admin/users/{handle}/invite-tokens endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add endpoint to list invite tokens for a specific user.

```rust
async fn list_user_invite_tokens(
    State(state): State<AppState>,
    Path(user_handle): Path<String>,
    session: Session,
) -> Result<Json<Vec<InviteTokenResponse>>, AppError> {
    require_manage_permissions(&session)?;

    let user_manager = state.user_manager.read().await;
    let tokens = user_manager.list_invite_tokens_for_user(&user_handle)?;

    let response: Vec<InviteTokenResponse> = tokens
        .into_iter()
        .map(|t| InviteTokenResponse {
            id: t.id,
            token: t.token,
            user_id: t.user_id,
            user_handle: t.user_handle,
            created_by: t.created_by,
            created_by_handle: t.created_by_handle,
            created: t.created,
            expires: t.expires,
            used_at: t.used_at,
        })
        .collect();

    Ok(Json(response))
}
```

---

### Task 6.5: Add GET /v1/admin/invite-tokens/{id} endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add endpoint to get a specific invite token by ID.

```rust
async fn get_invite_token(
    State(state): State<AppState>,
    Path(token_id): Path<usize>,
    session: Session,
) -> Result<Json<InviteTokenResponse>, AppError> {
    require_manage_permissions(&session)?;

    let user_manager = state.user_manager.read().await;
    let token = user_manager
        .get_invite_token(token_id)?
        .ok_or_else(|| AppError::NotFound("Invite token not found".to_string()))?;

    Ok(Json(InviteTokenResponse {
        id: token.id,
        token: token.token,
        user_id: token.user_id,
        user_handle: token.user_handle,
        created_by: token.created_by,
        created_by_handle: token.created_by_handle,
        created: token.created,
        expires: token.expires,
        used_at: token.used_at,
    }))
}
```

---

### Task 6.6: Add DELETE /v1/admin/invite-tokens/{id} endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add endpoint to revoke/delete an invite token.

```rust
async fn delete_invite_token(
    State(state): State<AppState>,
    Path(token_id): Path<usize>,
    session: Session,
) -> Result<StatusCode, AppError> {
    require_manage_permissions(&session)?;

    let user_manager = state.user_manager.read().await;
    user_manager.revoke_invite_token(token_id)?;

    Ok(StatusCode::NO_CONTENT)
}
```

---

### Task 6.7: Add POST /v1/auth/redeem-invite endpoint
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add public endpoint to redeem an invite token (no auth required).
- **Context**: Add to the auth routes section (not admin).

```rust
async fn redeem_invite_token(
    State(state): State<AppState>,
    Json(body): Json<RedeemInviteBody>,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), AppError> {
    let device_registration = DeviceRegistration {
        device_uuid: body.device_uuid,
        device_type: body.device_type.parse().unwrap_or(DeviceType::Unknown),
        device_name: body.device_name,
        os_info: body.os_info,
    };

    let mut user_manager = state.user_manager.write().await;
    let (auth_token, user_id) = user_manager
        .redeem_invite_token(&body.token, &device_registration)
        .map_err(|e| {
            // Map specific errors to appropriate HTTP status codes
            let msg = e.to_string();
            if msg.contains("Invalid") {
                AppError::NotFound(msg)
            } else if msg.contains("expired") || msg.contains("already been used") {
                AppError::Gone(msg)
            } else {
                AppError::Internal(msg)
            }
        })?;

    let user_handle = user_manager
        .get_user_handle(user_id)?
        .unwrap_or_default();

    // Set auth cookie (same as login)
    let mut headers = HeaderMap::new();
    let cookie = format!(
        "auth_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        auth_token.value.0,
        60 * 60 * 24 * 365 // 1 year
    );
    headers.insert(
        header::SET_COOKIE,
        cookie.parse().unwrap(),
    );

    Ok((
        StatusCode::OK,
        headers,
        Json(LoginResponse {
            user_id,
            user_handle,
            token: auth_token.value.0,
        }),
    ))
}
```

Note: May need to add `AppError::Gone` variant if it doesn't exist.

---

### Task 6.8: Register invite token routes
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs`
- **Description**: Add the new routes to the router.
- **Context**: Add to the route registration section.

Admin routes:
```rust
// In admin_routes builder
.route("/invite-tokens", get(list_all_invite_tokens))
.route("/invite-tokens/:id", get(get_invite_token).delete(delete_invite_token))
.route("/users/:handle/invite-token", post(create_invite_token_for_user))
.route("/users/:handle/invite-tokens", get(list_user_invite_tokens))
```

Auth routes (no auth required):
```rust
// In auth_routes builder
.route("/redeem-invite", post(redeem_invite_token))
```

---

### Task 6.9: Add AppError::Gone variant (if needed)
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/server.rs` (or error module)
- **Description**: Add a `Gone` (HTTP 410) error variant for expired/used tokens.

```rust
// In AppError enum
Gone(String),

// In impl IntoResponse for AppError
AppError::Gone(msg) => (StatusCode::GONE, msg).into_response(),
```

---

## Phase 7: Testing

### Task 7.1: Add unit tests for InviteTokenStore in sqlite_user_store.rs
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/sqlite_user_store.rs`
- **Description**: Add tests for the invite token store operations.

```rust
#[cfg(test)]
mod invite_token_tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (SqliteUserStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test.db");
        let store = SqliteUserStore::new(&temp_file_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_create_and_get_invite_token() {
        let (store, _temp_dir) = create_test_store();

        // Create a user first
        let user_id = store.create_user("testuser").unwrap();
        let admin_id = store.create_user("admin").unwrap();

        let expires = SystemTime::now() + std::time::Duration::from_secs(3600);
        let token_id = store
            .create_invite_token("test-token-value", user_id, admin_id, expires)
            .unwrap();

        // Get by ID
        let token = store.get_invite_token_by_id(token_id).unwrap().unwrap();
        assert_eq!(token.token, "test-token-value");
        assert_eq!(token.user_id, user_id);
        assert_eq!(token.created_by, admin_id);
        assert!(token.used_at.is_none());

        // Get by value
        let token = store.get_invite_token_by_value("test-token-value").unwrap().unwrap();
        assert_eq!(token.id, token_id);
    }

    #[test]
    fn test_mark_invite_token_used() {
        let (store, _temp_dir) = create_test_store();

        let user_id = store.create_user("testuser").unwrap();
        let expires = SystemTime::now() + std::time::Duration::from_secs(3600);
        let token_id = store
            .create_invite_token("test-token", user_id, user_id, expires)
            .unwrap();

        store.mark_invite_token_used(token_id).unwrap();

        let token = store.get_invite_token_by_id(token_id).unwrap().unwrap();
        assert!(token.used_at.is_some());
        assert!(token.is_used());
    }

    #[test]
    fn test_delete_invite_token() {
        let (store, _temp_dir) = create_test_store();

        let user_id = store.create_user("testuser").unwrap();
        let expires = SystemTime::now() + std::time::Duration::from_secs(3600);
        let token_id = store
            .create_invite_token("test-token", user_id, user_id, expires)
            .unwrap();

        store.delete_invite_token(token_id).unwrap();

        let token = store.get_invite_token_by_id(token_id).unwrap();
        assert!(token.is_none());
    }

    #[test]
    fn test_list_invite_tokens() {
        let (store, _temp_dir) = create_test_store();

        let user1_id = store.create_user("user1").unwrap();
        let user2_id = store.create_user("user2").unwrap();
        let expires = SystemTime::now() + std::time::Duration::from_secs(3600);

        store.create_invite_token("token1", user1_id, user1_id, expires).unwrap();
        store.create_invite_token("token2", user1_id, user1_id, expires).unwrap();
        store.create_invite_token("token3", user2_id, user1_id, expires).unwrap();

        // List all
        let all_tokens = store.list_all_invite_tokens().unwrap();
        assert_eq!(all_tokens.len(), 3);

        // List for user1
        let user1_tokens = store.list_invite_tokens_for_user(user1_id).unwrap();
        assert_eq!(user1_tokens.len(), 2);

        // List for user2
        let user2_tokens = store.list_invite_tokens_for_user(user2_id).unwrap();
        assert_eq!(user2_tokens.len(), 1);
    }

    #[test]
    fn test_invite_token_cascade_delete() {
        let (store, _temp_dir) = create_test_store();

        let user_id = store.create_user("testuser").unwrap();
        let expires = SystemTime::now() + std::time::Duration::from_secs(3600);
        store.create_invite_token("test-token", user_id, user_id, expires).unwrap();

        // Delete the user
        store.delete_user(user_id).unwrap();

        // Token should be deleted via cascade
        let tokens = store.list_all_invite_tokens().unwrap();
        assert!(tokens.is_empty());
    }
}
```

---

### Task 7.2: Add unit tests for UserManager invite token methods
- [ ] **Status**: Not started
- **File**: `catalog-server/src/user/user_manager.rs`
- **Description**: Add tests for the UserManager invite token methods.

---

### Task 7.3: Add integration tests for invite token API endpoints
- [ ] **Status**: Not started
- **File**: New test file or existing integration test file
- **Description**: Test the full flow of creating, listing, redeeming, and deleting invite tokens via the HTTP API.

---

## Phase 8: Metrics and Categorization

### Task 8.1: Add invite-token endpoint categorization
- [ ] **Status**: Not started
- **File**: `catalog-server/src/server/metrics.rs`
- **Description**: Update the `categorize_endpoint` function to handle invite token endpoints.

```rust
// In categorize_endpoint function
} else if path.starts_with("/v1/admin/invite-tokens") {
    "admin"
} else if path.starts_with("/v1/auth/redeem-invite") {
    "auth"
}
```

---

## Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1     | 2     | Database schema and migration |
| 2     | 3     | Data types |
| 3     | 3     | Store trait definitions |
| 4     | 10    | SQLite store implementation |
| 5     | 5     | UserManager methods |
| 6     | 9     | API endpoints |
| 7     | 3     | Testing |
| 8     | 1     | Metrics |

**Total tasks**: 36

---

## Notes

1. Tasks should be completed in order within each phase
2. Phases can be parallelized to some extent (e.g., Phase 2 and 3 can be done together)
3. After completing each task, run `cargo check` to verify compilation
4. After completing Phase 4, run the tests to verify store implementation
5. After completing Phase 6, manually test endpoints with curl or similar
