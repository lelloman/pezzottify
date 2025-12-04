# Device Entity Implementation Plan

## Overview

Add devices as first-class entities that persist across login/logout cycles. Each auth token requires a device reference, and login requires device information from the client.

## Key Design Decisions

- **Device as separate entity**: Devices have their own table and identity, independent of auth tokens
- **Auth tokens reference devices**: Each auth token requires a device reference (login requires device info)
- **Device survives tokens**: A device persists across logins/logouts and can be associated with multiple tokens over time
- **User association persists on logout**: `device.user_id` is NOT cleared on logout - only on user deletion (via `ON DELETE SET NULL`). This means a device always stays linked to its last user.
- **Per-user device limits**: Each user can have at most N devices (default: 50). When a new device is registered, oldest devices (by `last_seen`) are pruned to stay within limit. This prevents login/logout spam from clogging the database.
- **Migration**: Delete all existing tokens (no real users yet)
- **Device cleanup**: Two-tier pruning strategy:
  1. **Per-user ring-buffer**: Enforced on login, keeps max N devices per user
  2. **Orphan time-based**: Devices with `user_id IS NULL` (from deleted users) are pruned after configurable period (default: 90 days)

---

## Database Schema (Migration V8)

> **Note**: V7 already exists for `USER_SETTINGS_TABLE`. This migration is V8.

### New `device` table

```sql
CREATE TABLE device (
    id INTEGER PRIMARY KEY,
    device_uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER REFERENCES user(id) ON DELETE SET NULL,
    device_type TEXT NOT NULL,
    device_name TEXT,
    os_info TEXT,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);
CREATE INDEX idx_device_user ON device(user_id);
CREATE INDEX idx_device_uuid ON device(device_uuid);
```

### New `auth_token` table definition (AUTH_TOKEN_TABLE_V_8)

```sql
-- Migration steps:
DELETE FROM auth_token;

-- Recreate table with device_id (SQLite doesn't support ADD COLUMN with NOT NULL and FK well)
CREATE TABLE auth_token_new (
    user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
    value TEXT NOT NULL UNIQUE,
    created INTEGER DEFAULT (strftime('%s', 'now')),
    last_used INTEGER,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE
);
CREATE INDEX idx_auth_token_value ON auth_token_new(value);
CREATE INDEX idx_auth_token_device ON auth_token_new(device_id);

DROP TABLE auth_token;
ALTER TABLE auth_token_new RENAME TO auth_token;
```

**Note on `user_id` in device table**:
- `user_id` is set to the current user on each login via `associate_device_with_user`
- `user_id` is **NOT** cleared on logout - the device stays linked to its last user
- `user_id` becomes `NULL` only when the user is deleted (via `ON DELETE SET NULL`)
- This allows per-user device limits and prevents login/logout spam from creating orphaned devices

---

## Input Validation

All client-provided device fields must be validated:

| Field | Validation Rules |
|-------|------------------|
| `device_uuid` | Required, 8-64 chars, alphanumeric + hyphens only (UUID format encouraged) |
| `device_type` | Required, must be one of: "web", "android", "ios" (defaults to "unknown" if invalid) |
| `device_name` | Optional, max 100 chars, trimmed, no control characters |
| `os_info` | Optional, max 200 chars, trimmed, no control characters |

Validation errors return HTTP 400 with descriptive error message.

---

## Error Handling

### Login Error Responses

| Scenario | HTTP Status | Error Code | Description |
|----------|-------------|------------|-------------|
| Invalid credentials | 401 | `invalid_credentials` | Wrong username or password |
| Device validation failed | 400 | `invalid_device_info` | Device fields failed validation |
| Device registration failed | 500 | `device_registration_failed` | DB error during device upsert |
| Token creation failed | 500 | `token_creation_failed` | DB error during token creation |

Example error response:
```json
{
  "error": "invalid_device_info",
  "message": "device_uuid must be 8-64 characters"
}
```

---

## Model Changes

### New file: `catalog-server/src/user/device.rs`

```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use anyhow::{bail, Result};

// Validation constants
pub const DEVICE_UUID_MIN_LEN: usize = 8;
pub const DEVICE_UUID_MAX_LEN: usize = 64;
pub const DEVICE_NAME_MAX_LEN: usize = 100;
pub const OS_INFO_MAX_LEN: usize = 200;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Web,
    Android,
    Ios,
    Unknown,
}

impl DeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "web" => Self::Web,
            "android" => Self::Android,
            "ios" => Self::Ios,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    pub id: usize,
    pub device_uuid: String,
    pub user_id: Option<usize>,
    pub device_type: DeviceType,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
    pub first_seen: SystemTime,
    pub last_seen: SystemTime,
}

/// Input for registering/updating a device
#[derive(Clone, Debug)]
pub struct DeviceRegistration {
    pub device_uuid: String,
    pub device_type: DeviceType,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}

impl DeviceRegistration {
    /// Validates and sanitizes a DeviceRegistration from raw input.
    /// Returns error if validation fails.
    pub fn validate_and_sanitize(
        device_uuid: &str,
        device_type: &str,
        device_name: Option<&str>,
        os_info: Option<&str>,
    ) -> Result<Self> {
        // Validate device_uuid
        let device_uuid = device_uuid.trim();
        if device_uuid.len() < DEVICE_UUID_MIN_LEN || device_uuid.len() > DEVICE_UUID_MAX_LEN {
            bail!(
                "device_uuid must be {}-{} characters, got {}",
                DEVICE_UUID_MIN_LEN,
                DEVICE_UUID_MAX_LEN,
                device_uuid.len()
            );
        }
        if !device_uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            bail!("device_uuid must contain only alphanumeric characters and hyphens");
        }

        // Validate and sanitize device_name
        let device_name = device_name
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.len() > DEVICE_NAME_MAX_LEN {
                    &s[..DEVICE_NAME_MAX_LEN]
                } else {
                    s
                }
            })
            .map(|s| s.chars().filter(|c| !c.is_control()).collect::<String>());

        // Validate and sanitize os_info
        let os_info = os_info
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.len() > OS_INFO_MAX_LEN {
                    &s[..OS_INFO_MAX_LEN]
                } else {
                    s
                }
            })
            .map(|s| s.chars().filter(|c| !c.is_control()).collect::<String>());

        Ok(Self {
            device_uuid: device_uuid.to_string(),
            device_type: DeviceType::from_str(device_type),
            device_name,
            os_info,
        })
    }
}
```

### Modify: `catalog-server/src/user/auth.rs`

Add `device_id` field to `AuthToken`:

```rust
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthToken {
    pub user_id: usize,
    pub created: SystemTime,
    pub last_used: Option<SystemTime>,
    pub value: AuthTokenValue,
    pub device_id: usize,  // NEW
}
```

---

## Store Traits

### Add to: `catalog-server/src/user/user_store.rs`

> **Note**: Following codebase convention, add `DeviceStore` trait to existing `user_store.rs` rather than creating a separate file.

```rust
use crate::user::device::{Device, DeviceRegistration};

pub trait DeviceStore: Send + Sync {
    /// Register a new device or get existing one by device_uuid.
    /// If device exists, updates device_type/name/os_info and last_seen.
    /// Returns the device ID.
    fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize>;

    /// Get device by ID
    fn get_device(&self, device_id: usize) -> Result<Option<Device>>;

    /// Get device by UUID
    fn get_device_by_uuid(&self, device_uuid: &str) -> Result<Option<Device>>;

    /// Get all devices for a user
    fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>>;

    /// Update device's associated user (called on login)
    fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()>;

    /// Update last_seen timestamp
    fn touch_device(&self, device_id: usize) -> Result<()>;

    /// Prune orphaned devices (user_id IS NULL) that haven't been seen for the specified number of days.
    /// Returns the number of devices deleted.
    /// Note: This will cascade delete associated auth_tokens.
    fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize>;

    /// Enforce per-user device limit by pruning oldest devices (by last_seen).
    /// Called after associating a device with a user during login.
    /// Returns the number of devices deleted.
    fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize>;
}
```

### Update `FullUserStore` trait:

```rust
/// Combined trait for user storage with bandwidth, listening tracking, settings, and devices
pub trait FullUserStore: UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore {}

// Blanket implementation for any type that implements all user store traits
impl<T: UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore> FullUserStore for T {}
```

### Note on `UserAuthTokenStore`:

The existing `add_user_auth_token(&self, token: AuthToken)` method signature remains unchanged since `AuthToken` struct now includes `device_id`. No trait signature changes needed.

---

## SQLite Implementation

### Modify: `catalog-server/src/user/sqlite_user_store.rs`

#### 1. Add table constant `DEVICE_TABLE_V_8`:

```rust
const DEVICE_TABLE_V_8: Table = Table {
    name: "device",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true, is_unique = true),
        sqlite_column!("device_uuid", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!(
            "user_id",
            &SqlType::Integer,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::SetNull,
            })
        ),
        sqlite_column!("device_type", &SqlType::Text, non_null = true),
        sqlite_column!("device_name", &SqlType::Text),
        sqlite_column!("os_info", &SqlType::Text),
        sqlite_column!("first_seen", &SqlType::Integer, non_null = true, default_value = Some(DEFAULT_TIMESTAMP)),
        sqlite_column!("last_seen", &SqlType::Integer, non_null = true, default_value = Some(DEFAULT_TIMESTAMP)),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_device_user", "user_id"),
        ("idx_device_uuid", "device_uuid"),
    ],
};
```

#### 2. Add table constant `AUTH_TOKEN_TABLE_V_8`:

```rust
const AUTH_TOKEN_TABLE_V_8: Table = Table {
    name: "auth_token",
    columns: &[
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
        sqlite_column!("value", &SqlType::Text, non_null = true, is_unique = true),
        sqlite_column!("created", &SqlType::Integer, default_value = Some(DEFAULT_TIMESTAMP)),
        sqlite_column!("last_used", &SqlType::Integer),
        sqlite_column!(
            "device_id",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "device",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
    ],
    unique_constraints: &[],
    indices: &[
        ("idx_auth_token_value", "value"),
        ("idx_auth_token_device", "device_id"),
    ],
};
```

#### 3. Add `VersionedSchema` for V8:

```rust
VersionedSchema {
    version: 8,
    tables: &[
        USER_TABLE_V_0,
        LIKED_CONTENT_TABLE_V_2,
        AUTH_TOKEN_TABLE_V_8,  // Updated
        USER_PASSWORD_CREDENTIALS_V_0,
        USER_PLAYLIST_TABLE_V_3,
        USER_PLAYLIST_TRACKS_TABLE_V_3,
        USER_ROLE_TABLE_V_4,
        USER_EXTRA_PERMISSION_TABLE_V_4,
        BANDWIDTH_USAGE_TABLE_V_5,
        LISTENING_EVENTS_TABLE_V_6,
        USER_SETTINGS_TABLE_V_7,
        DEVICE_TABLE_V_8,  // New
    ],
    migration: Some(|conn: &Connection| {
        // Create device table first (auth_token will reference it)
        DEVICE_TABLE_V_8.create(&conn)?;

        // Delete all existing tokens (no real users yet)
        conn.execute("DELETE FROM auth_token", [])?;

        // Recreate auth_token with device_id column
        conn.execute("DROP TABLE auth_token", [])?;
        AUTH_TOKEN_TABLE_V_8.create(&conn)?;

        Ok(())
    }),
},
```

#### 4. Implement `DeviceStore` trait:

Key implementation notes:
- `register_or_update_device`: Use `INSERT ... ON CONFLICT(device_uuid) DO UPDATE`
- Token queries populate `device_id` from the table
- `prune_orphaned_devices`: Delete devices where `user_id IS NULL AND last_seen < now - inactive_days`
- `enforce_user_device_limit`:
  ```sql
  -- Delete oldest devices for user beyond the limit
  DELETE FROM device
  WHERE id IN (
      SELECT id FROM device
      WHERE user_id = ?
      ORDER BY last_seen DESC
      LIMIT -1 OFFSET ?  -- offset = max_devices
  )
  ```

---

## Server Changes

### Modify: `catalog-server/src/server/server.rs`

#### Update `LoginBody`:

```rust
#[derive(Deserialize, Debug)]
struct LoginBody {
    pub user_handle: String,
    pub password: String,
    pub device_uuid: String,
    pub device_type: String,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}
```

#### Add error response struct:

```rust
#[derive(Serialize)]
struct LoginErrorResponse {
    error: String,
    message: String,
}
```

#### Update login handler flow:

```rust
async fn login(
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    // 1. Validate device info first (fail fast)
    let device_registration = match DeviceRegistration::validate_and_sanitize(
        &body.device_uuid,
        &body.device_type,
        body.device_name.as_deref(),
        body.os_info.as_deref(),
    ) {
        Ok(reg) => reg,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(LoginErrorResponse {
                    error: "invalid_device_info".to_string(),
                    message: e.to_string(),
                }),
            ).into_response();
        }
    };

    // 2. Validate credentials
    // ... existing credential validation ...

    // 3. Register/update device
    let device_id = match locked_manager.register_or_update_device(&device_registration) {
        Ok(id) => id,
        Err(e) => {
            error!("Device registration failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginErrorResponse {
                    error: "device_registration_failed".to_string(),
                    message: "Failed to register device".to_string(),
                }),
            ).into_response();
        }
    };

    // 4. Associate device with user
    if let Err(e) = locked_manager.associate_device_with_user(device_id, credentials.user_id) {
        error!("Device association failed: {}", e);
        // Non-fatal, continue with login
    }

    // 5. Enforce per-user device limit (ring-buffer pruning)
    const MAX_DEVICES_PER_USER: usize = 50;
    if let Err(e) = locked_manager.enforce_user_device_limit(credentials.user_id, MAX_DEVICES_PER_USER) {
        error!("Device limit enforcement failed: {}", e);
        // Non-fatal, continue with login
    }

    // 6. Create auth token with device_id
    // 7. Return token
}
```

### Modify: `catalog-server/src/server/session.rs`

#### Update `Session` struct:

```rust
use crate::user::device::DeviceType;

#[derive(Debug)]
pub struct Session {
    pub user_id: usize,
    pub token: String,
    pub permissions: Vec<Permission>,
    pub device_id: usize,
    pub device_type: DeviceType,
}
```

#### Update session extraction:

The `AuthToken` now contains `device_id`. To get `device_type`, either:

**Option A (Recommended)**: Join device table when fetching token (single query):
```rust
// In get_user_auth_token implementation, join with device table
// Return extended AuthToken or separate struct with device_type
```

**Option B**: Separate query for device info:
```rust
// After getting auth_token, query device by device_id
let device = user_manager.get_device(auth_token.device_id)?;
```

For performance, Option A is preferred. Consider adding `device_type` to `AuthToken` or creating an `AuthTokenWithDevice` struct.

---

## UserManager Changes

### Modify: `catalog-server/src/user/user_manager.rs`

#### Add device-related methods:

```rust
impl UserManager {
    /// Register or update a device. Returns device ID.
    pub fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize> {
        self.user_store.register_or_update_device(registration)
    }

    /// Associate a device with a user (called on login).
    pub fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()> {
        self.user_store.associate_device_with_user(device_id, user_id)
    }

    /// Get device by ID.
    pub fn get_device(&self, device_id: usize) -> Result<Option<Device>> {
        self.user_store.get_device(device_id)
    }

    /// Get all devices for a user.
    pub fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>> {
        self.user_store.get_user_devices(user_id)
    }

    /// Prune orphaned devices (user_id IS NULL) older than specified days.
    pub fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize> {
        self.user_store.prune_orphaned_devices(inactive_for_days)
    }

    /// Enforce per-user device limit by pruning oldest devices.
    pub fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize> {
        self.user_store.enforce_user_device_limit(user_id, max_devices)
    }
}
```

#### Update `generate_auth_token`:

```rust
pub fn generate_auth_token(&mut self, credentials: &UserAuthCredentials, device_id: usize) -> Result<AuthToken> {
    let token = AuthToken {
        user_id: credentials.user_id,
        value: AuthTokenValue::generate(),
        created: SystemTime::now(),
        last_used: None,
        device_id,  // NEW
    };
    self.user_store.add_user_auth_token(token.clone())?;
    Ok(token)
}
```

---

## Files to Modify

- `catalog-server/src/user/mod.rs` - Export new device module
- `catalog-server/src/user/auth.rs` - Add `device_id` to `AuthToken`
- `catalog-server/src/user/user_store.rs` - Add `DeviceStore` trait, update `FullUserStore`
- `catalog-server/src/user/sqlite_user_store.rs` - Migration V8, implement `DeviceStore`, update token methods
- `catalog-server/src/user/user_manager.rs` - Add device-aware methods, update `generate_auth_token`
- `catalog-server/src/server/server.rs` - Update `LoginBody`, login handler, add error responses
- `catalog-server/src/server/session.rs` - Add device info to `Session`

## Files to Create

- `catalog-server/src/user/device.rs` - `DeviceType`, `Device`, `DeviceRegistration` with validation

---

## Implementation Order

1. Create `device.rs` with types and validation
2. Add `DeviceStore` trait to `user_store.rs`
3. Update `FullUserStore` trait combination
4. Add table constants (`DEVICE_TABLE_V_8`, `AUTH_TOKEN_TABLE_V_8`) to `sqlite_user_store.rs`
5. Add `VersionedSchema` V8 with migration
6. Implement `DeviceStore` in `SqliteUserStore`
7. Update `AuthToken` struct in `auth.rs` (add `device_id`)
8. Update `SqliteUserStore` token methods to handle `device_id`
9. Add device methods to `UserManager`
10. Update `generate_auth_token` in `UserManager` to take `device_id`
11. Update `LoginBody` in `server.rs`
12. Update login handler with validation and error responses
13. Update `Session` struct in `session.rs`
14. Update session extraction to populate device info
15. Export new modules in `mod.rs`

---

## Testing

### Unit Tests

#### `device.rs` tests:
1. `DeviceType::from_str` - valid types ("web", "android", "ios")
2. `DeviceType::from_str` - invalid/unknown types default to `Unknown`
3. `DeviceType::as_str` - roundtrip conversion
4. `DeviceRegistration::validate_and_sanitize` - valid input
5. `DeviceRegistration::validate_and_sanitize` - device_uuid too short (< 8 chars)
6. `DeviceRegistration::validate_and_sanitize` - device_uuid too long (> 64 chars)
7. `DeviceRegistration::validate_and_sanitize` - device_uuid with invalid characters
8. `DeviceRegistration::validate_and_sanitize` - device_name truncation at 100 chars
9. `DeviceRegistration::validate_and_sanitize` - os_info truncation at 200 chars
10. `DeviceRegistration::validate_and_sanitize` - control characters stripped
11. `DeviceRegistration::validate_and_sanitize` - whitespace trimming
12. `DeviceRegistration::validate_and_sanitize` - empty optional fields become None

#### `DeviceStore` tests (in `sqlite_user_store.rs`):
1. `register_or_update_device` - new device creation
2. `register_or_update_device` - existing device update (same uuid)
3. `register_or_update_device` - updates last_seen on re-registration
4. `get_device` - existing device
5. `get_device` - non-existent device returns None
6. `get_device_by_uuid` - existing device
7. `get_device_by_uuid` - non-existent uuid returns None
8. `get_user_devices` - returns all devices for user
9. `get_user_devices` - empty list for user with no devices
10. `associate_device_with_user` - updates user_id
11. `associate_device_with_user` - device can change users
12. `touch_device` - updates last_seen timestamp
13. `prune_orphaned_devices` - removes old orphaned devices (user_id IS NULL)
14. `prune_orphaned_devices` - keeps recent orphaned devices
15. `prune_orphaned_devices` - does not affect devices with user_id set
16. `prune_orphaned_devices` - cascades to delete auth_tokens
17. `enforce_user_device_limit` - removes oldest devices when over limit
18. `enforce_user_device_limit` - keeps devices when under limit
19. `enforce_user_device_limit` - removes correct number of devices (count - max)
20. `enforce_user_device_limit` - does not affect other users' devices
21. `enforce_user_device_limit` - cascades to delete auth_tokens

#### `AuthToken` with device tests:
1. `add_user_auth_token` - token created with device_id
2. `get_user_auth_token` - returns token with device_id populated
3. Token deletion cascades when device deleted
4. Multiple tokens can reference same device

### Integration Tests

#### Login flow tests:
1. Login with valid credentials and valid device info - success
2. Login with valid credentials but missing device_uuid - 400 error
3. Login with valid credentials but invalid device_uuid format - 400 error
4. Login with invalid credentials and valid device info - 401 error
5. Login creates/updates device record
6. Login associates device with user
7. Multiple logins from same device reuse device record
8. Login from same device with different user updates device.user_id

#### Session tests:
1. Session extraction populates device_id and device_type
2. Session with deleted device fails gracefully (token cascade deleted)

#### Migration tests:
1. V7 to V8 migration creates device table
2. V7 to V8 migration recreates auth_token with device_id
3. V7 to V8 migration deletes existing tokens
4. Fresh V8 database has correct schema

#### Device lifecycle tests:
1. Device persists across logout/login cycle
2. Device persists when all tokens deleted
3. Device.user_id remains set after logout (not cleared)
4. Device.user_id set to NULL when user deleted
5. Orphaned device pruning works correctly (only affects user_id IS NULL)
6. Device pruning cascades to token deletion
7. Per-user device limit enforced on login
8. Old devices pruned when user exceeds device limit
9. Device limit pruning does not affect other users

#### Concurrent access tests:
1. Concurrent logins from same device don't conflict
2. Concurrent device registration with same uuid is idempotent

---

## Future Considerations

1. **Device management API**: Allow users to view/revoke their devices
2. **Device limits**: Maximum devices per user
3. **Device verification**: Require re-authentication when switching devices
4. **Push notifications**: Device tokens for push notification delivery
