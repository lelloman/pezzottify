# Device Entity Implementation - Detailed TODO List

This document breaks down the Device Entity Implementation Plan into small, sequential, and actionable tasks.

---

## Status Legend

| Symbol | Meaning |
|--------|---------|
| `[ ]` | Not started |
| `[~]` | In progress |
| `[x]` | Complete |

---

## Progress Overview

| Phase | Description | Tasks | Status |
|-------|-------------|-------|--------|
| 1 | Core Types and Validation | 9 | [ ] |
| 2 | Store Trait Definitions | 2 | [ ] |
| 3 | AuthToken Changes | 1 | [ ] |
| 4 | SQLite Schema and Migration | 3 | [ ] |
| 5 | DeviceStore Implementation | 9 | [ ] |
| 6 | Update Auth Token Persistence | 2 | [ ] |
| 7 | UserManager Changes | 3 | [ ] |
| 8 | Server Changes | 7 | [ ] |
| 9 | Session Changes | 3 | [ ] |
| 10 | Testing | 6 | [ ] |
| 11 | Final Cleanup and Verification | 5 | [ ] |
| **Total** | | **50** | |

---

## Phase 1: Core Types and Validation

### [x] Task 1.1: Create device.rs module file
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
1. Create the new file `device.rs` in `catalog-server/src/user/`
2. Add the following imports:
   ```rust
   use serde::{Deserialize, Serialize};
   use std::time::SystemTime;
   use anyhow::{bail, Result};
   ```

**Design**: This module will contain all device-related types and validation logic, keeping device concerns isolated from auth concerns.

---

### [x] Task 1.2: Add validation constants
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
Add these constants at the top of the file:
```rust
pub const DEVICE_UUID_MIN_LEN: usize = 8;
pub const DEVICE_UUID_MAX_LEN: usize = 64;
pub const DEVICE_NAME_MAX_LEN: usize = 100;
pub const OS_INFO_MAX_LEN: usize = 200;
```

**Design**: Constants are public so they can be used in error messages and potentially by API documentation. Values chosen to be permissive but prevent abuse.

---

### [x] Task 1.3: Implement DeviceType enum
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
```rust
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
```

**Design**:
- `Unknown` variant handles invalid/future device types gracefully
- `from_str` is lenient (returns Unknown for invalid input) because device_type is not security-critical
- `as_str` returns static strings for efficient DB storage
- Serde attributes enable JSON serialization with lowercase names

---

### [x] Task 1.4: Implement Device struct
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
```rust
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
```

**Design**:
- `id` is the internal DB primary key
- `device_uuid` is the client-provided identifier (must be unique)
- `user_id` is optional because it becomes NULL when user is deleted (ON DELETE SET NULL)
- `first_seen` and `last_seen` track device lifetime for pruning decisions

---

### [x] Task 1.5: Implement DeviceRegistration struct
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
```rust
#[derive(Clone, Debug)]
pub struct DeviceRegistration {
    pub device_uuid: String,
    pub device_type: DeviceType,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}
```

**Design**: This is the input type for device registration, separate from `Device` because it doesn't include server-assigned fields (id, user_id, timestamps).

---

### [x] Task 1.6: Implement DeviceRegistration::validate_and_sanitize
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
```rust
impl DeviceRegistration {
    pub fn validate_and_sanitize(
        device_uuid: &str,
        device_type: &str,
        device_name: Option<&str>,
        os_info: Option<&str>,
    ) -> Result<Self> {
        // 1. Validate device_uuid
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

        // 2. Validate and sanitize device_name (optional, truncate, strip control chars)
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

        // 3. Validate and sanitize os_info (optional, truncate, strip control chars)
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

**Design**:
- `device_uuid` is strictly validated (required, length limits, character whitelist) because it's used as a unique identifier
- `device_name` and `os_info` are sanitized but permissive (truncate instead of reject)
- Control characters stripped to prevent display issues
- Returns `Result` for validation errors that should return HTTP 400

---

### [x] Task 1.7: Export device module from mod.rs
**File**: `catalog-server/src/user/mod.rs`

**Implementation**:
Add to existing exports:
```rust
pub mod device;
```

**Design**: Public export allows other modules (server, user_manager) to use device types.

---

### [x] Task 1.8: Add unit tests for DeviceType
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
Add at the bottom of the file:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_from_str_valid() {
        assert_eq!(DeviceType::from_str("web"), DeviceType::Web);
        assert_eq!(DeviceType::from_str("android"), DeviceType::Android);
        assert_eq!(DeviceType::from_str("ios"), DeviceType::Ios);
        assert_eq!(DeviceType::from_str("WEB"), DeviceType::Web); // case insensitive
        assert_eq!(DeviceType::from_str("Android"), DeviceType::Android);
    }

    #[test]
    fn test_device_type_from_str_invalid() {
        assert_eq!(DeviceType::from_str(""), DeviceType::Unknown);
        assert_eq!(DeviceType::from_str("windows"), DeviceType::Unknown);
        assert_eq!(DeviceType::from_str("invalid"), DeviceType::Unknown);
    }

    #[test]
    fn test_device_type_as_str_roundtrip() {
        assert_eq!(DeviceType::from_str(DeviceType::Web.as_str()), DeviceType::Web);
        assert_eq!(DeviceType::from_str(DeviceType::Android.as_str()), DeviceType::Android);
        assert_eq!(DeviceType::from_str(DeviceType::Ios.as_str()), DeviceType::Ios);
        assert_eq!(DeviceType::from_str(DeviceType::Unknown.as_str()), DeviceType::Unknown);
    }
}
```

---

### [x] Task 1.9: Add unit tests for DeviceRegistration validation
**File**: `catalog-server/src/user/device.rs`

**Implementation**:
Add to the test module:
```rust
#[test]
fn test_validate_valid_input() {
    let result = DeviceRegistration::validate_and_sanitize(
        "test-uuid-1234",
        "android",
        Some("My Phone"),
        Some("Android 14"),
    );
    assert!(result.is_ok());
    let reg = result.unwrap();
    assert_eq!(reg.device_uuid, "test-uuid-1234");
    assert_eq!(reg.device_type, DeviceType::Android);
    assert_eq!(reg.device_name, Some("My Phone".to_string()));
    assert_eq!(reg.os_info, Some("Android 14".to_string()));
}

#[test]
fn test_validate_uuid_too_short() {
    let result = DeviceRegistration::validate_and_sanitize("short", "web", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("device_uuid"));
}

#[test]
fn test_validate_uuid_too_long() {
    let long_uuid = "a".repeat(65);
    let result = DeviceRegistration::validate_and_sanitize(&long_uuid, "web", None, None);
    assert!(result.is_err());
}

#[test]
fn test_validate_uuid_invalid_chars() {
    let result = DeviceRegistration::validate_and_sanitize("uuid with spaces!", "web", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("alphanumeric"));
}

#[test]
fn test_validate_device_name_truncation() {
    let long_name = "x".repeat(150);
    let result = DeviceRegistration::validate_and_sanitize("valid-uuid", "web", Some(&long_name), None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().device_name.unwrap().len(), DEVICE_NAME_MAX_LEN);
}

#[test]
fn test_validate_os_info_truncation() {
    let long_info = "y".repeat(250);
    let result = DeviceRegistration::validate_and_sanitize("valid-uuid", "web", None, Some(&long_info));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().os_info.unwrap().len(), OS_INFO_MAX_LEN);
}

#[test]
fn test_validate_control_chars_stripped() {
    let result = DeviceRegistration::validate_and_sanitize(
        "valid-uuid",
        "web",
        Some("Name\x00With\x1FControl"),
        Some("OS\nInfo"),
    );
    assert!(result.is_ok());
    let reg = result.unwrap();
    assert_eq!(reg.device_name, Some("NameWithControl".to_string()));
    assert_eq!(reg.os_info, Some("OSInfo".to_string()));
}

#[test]
fn test_validate_whitespace_trimming() {
    let result = DeviceRegistration::validate_and_sanitize(
        "  valid-uuid  ",
        "web",
        Some("  trimmed  "),
        None,
    );
    assert!(result.is_ok());
    let reg = result.unwrap();
    assert_eq!(reg.device_uuid, "valid-uuid");
    assert_eq!(reg.device_name, Some("trimmed".to_string()));
}

#[test]
fn test_validate_empty_optional_becomes_none() {
    let result = DeviceRegistration::validate_and_sanitize("valid-uuid", "web", Some(""), Some("   "));
    assert!(result.is_ok());
    let reg = result.unwrap();
    assert!(reg.device_name.is_none());
    assert!(reg.os_info.is_none());
}
```

---

## Phase 2: Store Trait Definitions

### [x] Task 2.1: Add DeviceStore trait to user_store.rs
**File**: `catalog-server/src/user/user_store.rs`

**Implementation**:
1. Add import at top:
   ```rust
   use crate::user::device::{Device, DeviceRegistration};
   ```

2. Add the trait definition:
   ```rust
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

       /// Prune orphaned devices (user_id IS NULL) that haven't been seen for the specified days.
       /// Returns the number of devices deleted.
       fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize>;

       /// Enforce per-user device limit by pruning oldest devices (by last_seen).
       /// Called after associating a device with a user during login.
       /// Returns the number of devices deleted.
       fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize>;
   }
   ```

**Design**:
- All methods return `Result` for error propagation
- `register_or_update_device` uses upsert semantics to handle concurrent registrations
- Pruning methods return count for logging/monitoring purposes
- Methods are idempotent where possible (touch_device, associate_device_with_user)

---

### [x] Task 2.2: Update FullUserStore trait bound
**File**: `catalog-server/src/user/user_store.rs`

**Implementation**:
Update the trait definition:
```rust
/// Combined trait for user storage with bandwidth, listening tracking, settings, and devices
pub trait FullUserStore: UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore {}

// Update blanket implementation
impl<T: UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore> FullUserStore for T {}
```

**Design**: Adding `DeviceStore` to the combined trait ensures all implementations provide device functionality.

---

## Phase 3: AuthToken Changes

### [x] Task 3.1: Add device_id field to AuthToken struct
**File**: `catalog-server/src/user/auth.rs`

**Implementation**:
Update the `AuthToken` struct:
```rust
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthToken {
    pub user_id: usize,
    pub created: SystemTime,
    pub last_used: Option<SystemTime>,
    pub value: AuthTokenValue,
    pub device_id: usize,  // NEW FIELD
}
```

**Design**:
- `device_id` is required (not Option) because every token MUST be associated with a device after migration
- This is the foreign key to the device table

---

## Phase 4: SQLite Schema and Migration

### [x] Task 4.1: Add DEVICE_TABLE_V_8 constant
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Add after existing table constants:
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

**Design**:
- `ON DELETE SET NULL` for user_id: devices persist when user deleted, just become orphaned
- Indices on user_id (for get_user_devices) and device_uuid (for lookups during login)
- Timestamps use INTEGER (Unix epoch seconds) for SQLite compatibility

---

### [x] Task 4.2: Add AUTH_TOKEN_TABLE_V_8 constant
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Add after DEVICE_TABLE_V_8:
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

**Design**:
- `device_id` with `ON DELETE CASCADE`: when device is deleted, all its tokens are deleted
- This cascades correctly: delete device → delete tokens; delete user → set device.user_id NULL but device persists

---

### [x] Task 4.3: Add VersionedSchema V8 entry
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Add to the VERSIONED_SCHEMAS array:
```rust
VersionedSchema {
    version: 8,
    tables: &[
        USER_TABLE_V_0,
        LIKED_CONTENT_TABLE_V_2,
        AUTH_TOKEN_TABLE_V_8,  // Updated from V0
        USER_PASSWORD_CREDENTIALS_V_0,
        USER_PLAYLIST_TABLE_V_3,
        USER_PLAYLIST_TRACKS_TABLE_V_3,
        USER_ROLE_TABLE_V_4,
        USER_EXTRA_PERMISSION_TABLE_V_4,
        BANDWIDTH_USAGE_TABLE_V_5,
        LISTENING_EVENTS_TABLE_V_6,
        USER_SETTINGS_TABLE_V_7,
        DEVICE_TABLE_V_8,  // New table
    ],
    migration: Some(|conn: &Connection| {
        // Step 1: Create device table first (auth_token will reference it)
        DEVICE_TABLE_V_8.create(&conn)?;

        // Step 2: Delete all existing tokens (no real users yet, per plan)
        conn.execute("DELETE FROM auth_token", [])?;

        // Step 3: Recreate auth_token with device_id column
        // SQLite doesn't support ADD COLUMN with NOT NULL and FK well
        conn.execute("DROP TABLE auth_token", [])?;
        AUTH_TOKEN_TABLE_V_8.create(&conn)?;

        Ok(())
    }),
},
```

**Design**:
- Migration order is critical: device table must exist before auth_token references it
- Dropping all tokens is acceptable per plan (no real users in production yet)
- Using DROP TABLE + CREATE TABLE because SQLite's ALTER TABLE is limited

---

## Phase 5: DeviceStore Implementation

### [x] Task 5.1: Add device module import to sqlite_user_store.rs
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Add to imports:
```rust
use crate::user::device::{Device, DeviceRegistration, DeviceType};
```

---

### [x] Task 5.2: Implement register_or_update_device
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
impl DeviceStore for SqliteUserStore {
    fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize> {
        let conn = self.connection.lock().unwrap();
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

        Ok(device_id)
    }
```

**Design**:
- Uses SQLite upsert (`ON CONFLICT ... DO UPDATE`) for atomic operation
- Always updates `last_seen` on re-registration
- Returns device ID for use in token creation
- Thread-safe via Mutex on connection

---

### [x] Task 5.3: Implement get_device
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn get_device(&self, device_id: usize) -> Result<Option<Device>> {
        let conn = self.connection.lock().unwrap();
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
                    first_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
```

**Design**:
- Returns `Option<Device>` to handle not-found case without error
- Converts INTEGER timestamps back to SystemTime

---

### [x] Task 5.4: Implement get_device_by_uuid
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn get_device_by_uuid(&self, device_uuid: &str) -> Result<Option<Device>> {
        let conn = self.connection.lock().unwrap();
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
                    first_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                    last_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
                })
            },
        );

        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
```

---

### [x] Task 5.5: Implement get_user_devices
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, device_uuid, user_id, device_type, device_name, os_info, first_seen, last_seen
             FROM device WHERE user_id = ?1 ORDER BY last_seen DESC"
        )?;

        let devices = stmt.query_map(params![user_id], |row| {
            Ok(Device {
                id: row.get(0)?,
                device_uuid: row.get(1)?,
                user_id: row.get(2)?,
                device_type: DeviceType::from_str(&row.get::<_, String>(3)?),
                device_name: row.get(4)?,
                os_info: row.get(5)?,
                first_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(6)? as u64),
                last_seen: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(row.get::<_, i64>(7)? as u64),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(devices)
    }
```

**Design**:
- Ordered by `last_seen DESC` to show most recently used devices first
- Returns empty Vec for users with no devices (not an error)

---

### [x] Task 5.6: Implement associate_device_with_user
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "UPDATE device SET user_id = ?1, last_seen = ?2 WHERE id = ?3",
            params![user_id, now, device_id],
        )?;
        Ok(())
    }
```

**Design**:
- Also updates `last_seen` since this is called during login
- Idempotent: can be called multiple times safely
- Per plan: user_id is NOT cleared on logout, only updated on login

---

### [x] Task 5.7: Implement touch_device
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn touch_device(&self, device_id: usize) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "UPDATE device SET last_seen = ?1 WHERE id = ?2",
            params![now, device_id],
        )?;
        Ok(())
    }
```

**Design**: Simple timestamp update for keeping track of active devices.

---

### [x] Task 5.8: Implement prune_orphaned_devices
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize> {
        let conn = self.connection.lock().unwrap();
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (inactive_for_days as i64 * 24 * 60 * 60);

        let deleted = conn.execute(
            "DELETE FROM device WHERE user_id IS NULL AND last_seen < ?1",
            params![cutoff],
        )?;

        Ok(deleted)
    }
```

**Design**:
- Only deletes devices with `user_id IS NULL` (orphaned from deleted users)
- Uses time-based cutoff to avoid deleting recently-used devices
- Cascade delete handles auth_tokens automatically
- Returns count for logging

---

### [x] Task 5.9: Implement enforce_user_device_limit
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
    fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize> {
        let conn = self.connection.lock().unwrap();

        // Delete oldest devices for this user beyond the limit
        // Uses subquery to find devices to delete
        let deleted = conn.execute(
            "DELETE FROM device
             WHERE id IN (
                 SELECT id FROM device
                 WHERE user_id = ?1
                 ORDER BY last_seen DESC
                 LIMIT -1 OFFSET ?2
             )",
            params![user_id, max_devices],
        )?;

        Ok(deleted)
    }
}  // End of DeviceStore impl
```

**Design**:
- `LIMIT -1 OFFSET max_devices` selects all devices after the first N (sorted by most recent)
- Ring-buffer behavior: keeps the N most recently used devices
- Cascade delete handles auth_tokens
- Called after login to enforce limit

---

## Phase 6: Update Auth Token Persistence

### [ ] Task 6.1: Update add_user_auth_token to include device_id
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Find the `add_user_auth_token` method and update:
```rust
fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
    let conn = self.connection.lock().unwrap();
    let created = token.created
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO auth_token (user_id, value, created, device_id) VALUES (?1, ?2, ?3, ?4)",
        params![
            token.user_id,
            token.value.value(),
            created,
            token.device_id,  // NEW
        ],
    )?;
    Ok(())
}
```

---

### [ ] Task 6.2: Update get_user_auth_token to return device_id
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Find the `get_user_auth_token` method and update:
```rust
fn get_user_auth_token(&self, token_value: &str) -> Result<Option<AuthToken>> {
    let conn = self.connection.lock().unwrap();
    let result = conn.query_row(
        "SELECT user_id, value, created, last_used, device_id FROM auth_token WHERE value = ?1",
        params![token_value],
        |row| {
            let created_secs: i64 = row.get(2)?;
            let last_used_secs: Option<i64> = row.get(3)?;
            Ok(AuthToken {
                user_id: row.get(0)?,
                value: AuthTokenValue::new(row.get(1)?),
                created: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(created_secs as u64),
                last_used: last_used_secs.map(|s| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s as u64)),
                device_id: row.get(4)?,  // NEW
            })
        },
    );

    match result {
        Ok(token) => Ok(Some(token)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
```

---

## Phase 7: UserManager Changes

### [ ] Task 7.1: Add device imports to user_manager.rs
**File**: `catalog-server/src/user/user_manager.rs`

**Implementation**:
Add to imports:
```rust
use crate::user::device::{Device, DeviceRegistration};
```

---

### [ ] Task 7.2: Add device delegation methods to UserManager
**File**: `catalog-server/src/user/user_manager.rs`

**Implementation**:
Add these methods to the `UserManager` impl block:
```rust
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
```

**Design**: Simple delegation to store layer, keeping UserManager as the API surface.

---

### [ ] Task 7.3: Update generate_auth_token signature and implementation
**File**: `catalog-server/src/user/user_manager.rs`

**Implementation**:
Find `generate_auth_token` and update:
```rust
pub fn generate_auth_token(&mut self, credentials: &UserAuthCredentials, device_id: usize) -> Result<AuthToken> {
    let token = AuthToken {
        user_id: credentials.user_id,
        value: AuthTokenValue::generate(),
        created: SystemTime::now(),
        last_used: None,
        device_id,  // NEW PARAMETER
    };
    self.user_store.add_user_auth_token(token.clone())?;
    Ok(token)
}
```

**Design**: Now requires device_id, ensuring tokens are always associated with a device.

---

## Phase 8: Server Changes

### [ ] Task 8.1: Update LoginBody struct
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Find `LoginBody` and update:
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

---

### [ ] Task 8.2: Add LoginErrorResponse struct
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Add near other response structs:
```rust
#[derive(Serialize)]
struct LoginErrorResponse {
    error: String,
    message: String,
}
```

---

### [ ] Task 8.3: Add device imports to server.rs
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Add to imports:
```rust
use crate::user::device::DeviceRegistration;
```

---

### [ ] Task 8.4: Add MAX_DEVICES_PER_USER constant
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Add near top of file:
```rust
const MAX_DEVICES_PER_USER: usize = 50;
```

---

### [ ] Task 8.5: Update login handler - Part 1: Device validation
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Find the login handler and add device validation at the start:
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

    // ... rest of login logic (credential validation) ...
```

**Design**: Validate device info before checking credentials to fail fast on malformed requests.

---

### [ ] Task 8.6: Update login handler - Part 2: Device registration
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
After successful credential validation, add:
```rust
    // ... after credential validation succeeds, before token generation ...

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

    // 5. Enforce per-user device limit
    if let Err(e) = locked_manager.enforce_user_device_limit(credentials.user_id, MAX_DEVICES_PER_USER) {
        error!("Device limit enforcement failed: {}", e);
        // Non-fatal, continue with login
    }

    // 6. Generate auth token with device_id
    let token = locked_manager.generate_auth_token(&credentials, device_id)?;
    // ... rest of token response ...
```

**Design**:
- Device registration failure is fatal (can't create token without device)
- Association and limit enforcement are non-fatal (logged but don't fail login)
- Device limit enforced after login to prevent spam

---

### [ ] Task 8.7: Update generate_auth_token call site
**File**: `catalog-server/src/server/server.rs`

**Implementation**:
Update any remaining calls to `generate_auth_token` to pass `device_id`:
```rust
let token = locked_manager.generate_auth_token(&credentials, device_id)?;
```

---

## Phase 9: Session Changes

### [ ] Task 9.1: Add device imports to session.rs
**File**: `catalog-server/src/server/session.rs`

**Implementation**:
Add to imports:
```rust
use crate::user::device::DeviceType;
```

---

### [ ] Task 9.2: Update Session struct
**File**: `catalog-server/src/server/session.rs`

**Implementation**:
```rust
#[derive(Debug)]
pub struct Session {
    pub user_id: usize,
    pub token: String,
    pub permissions: Vec<Permission>,
    pub device_id: usize,
    pub device_type: DeviceType,
}
```

---

### [ ] Task 9.3: Update session extraction to populate device info
**File**: `catalog-server/src/server/session.rs`

**Implementation**:
In the session extraction logic (where AuthToken is loaded), add device lookup:
```rust
// After getting auth_token from store...
let device = user_manager
    .get_device(auth_token.device_id)?
    .ok_or_else(|| anyhow!("Device not found for token"))?;

let session = Session {
    user_id: auth_token.user_id,
    token: auth_token.value.value().to_string(),
    permissions,  // existing permissions logic
    device_id: device.id,
    device_type: device.device_type,
};
```

**Design**:
- Device lookup is a separate query (Option B from plan)
- If device doesn't exist (was deleted), session extraction fails gracefully
- Token cascade delete should prevent this case, but we handle it defensively

---

## Phase 10: Testing

### [ ] Task 10.1: Add DeviceStore unit tests - Registration
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
Add to the test module:
```rust
#[test]
fn test_register_new_device() {
    let store = create_test_store();
    let reg = DeviceRegistration {
        device_uuid: "test-uuid-12345".to_string(),
        device_type: DeviceType::Android,
        device_name: Some("Test Phone".to_string()),
        os_info: Some("Android 14".to_string()),
    };

    let device_id = store.register_or_update_device(&reg).unwrap();
    assert!(device_id > 0);

    let device = store.get_device(device_id).unwrap().unwrap();
    assert_eq!(device.device_uuid, "test-uuid-12345");
    assert_eq!(device.device_type, DeviceType::Android);
    assert_eq!(device.device_name, Some("Test Phone".to_string()));
}

#[test]
fn test_register_existing_device_updates() {
    let store = create_test_store();
    let reg1 = DeviceRegistration {
        device_uuid: "test-uuid-12345".to_string(),
        device_type: DeviceType::Android,
        device_name: Some("Old Name".to_string()),
        os_info: None,
    };
    let id1 = store.register_or_update_device(&reg1).unwrap();

    let reg2 = DeviceRegistration {
        device_uuid: "test-uuid-12345".to_string(),
        device_type: DeviceType::Android,
        device_name: Some("New Name".to_string()),
        os_info: Some("Updated OS".to_string()),
    };
    let id2 = store.register_or_update_device(&reg2).unwrap();

    assert_eq!(id1, id2);  // Same device
    let device = store.get_device(id1).unwrap().unwrap();
    assert_eq!(device.device_name, Some("New Name".to_string()));
    assert_eq!(device.os_info, Some("Updated OS".to_string()));
}
```

---

### [ ] Task 10.2: Add DeviceStore unit tests - User association
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
#[test]
fn test_associate_device_with_user() {
    let store = create_test_store();
    // Create a user first
    let user_id = store.add_user("testuser", "test@example.com").unwrap();

    let reg = DeviceRegistration {
        device_uuid: "test-uuid-12345".to_string(),
        device_type: DeviceType::Web,
        device_name: None,
        os_info: None,
    };
    let device_id = store.register_or_update_device(&reg).unwrap();

    // Initially no user
    let device = store.get_device(device_id).unwrap().unwrap();
    assert!(device.user_id.is_none());

    // Associate with user
    store.associate_device_with_user(device_id, user_id).unwrap();

    let device = store.get_device(device_id).unwrap().unwrap();
    assert_eq!(device.user_id, Some(user_id));
}

#[test]
fn test_get_user_devices() {
    let store = create_test_store();
    let user_id = store.add_user("testuser", "test@example.com").unwrap();

    // Register and associate multiple devices
    for i in 0..3 {
        let reg = DeviceRegistration {
            device_uuid: format!("uuid-{}", i),
            device_type: DeviceType::Android,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();
        store.associate_device_with_user(device_id, user_id).unwrap();
    }

    let devices = store.get_user_devices(user_id).unwrap();
    assert_eq!(devices.len(), 3);
}
```

---

### [ ] Task 10.3: Add DeviceStore unit tests - Pruning
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
#[test]
fn test_prune_orphaned_devices() {
    let store = create_test_store();

    // Create an orphaned device (no user_id)
    let reg = DeviceRegistration {
        device_uuid: "orphan-uuid".to_string(),
        device_type: DeviceType::Web,
        device_name: None,
        os_info: None,
    };
    let device_id = store.register_or_update_device(&reg).unwrap();

    // Prune with 0 days should delete it (since it was just created)
    // For proper testing, would need to mock time or set old timestamp
    let deleted = store.prune_orphaned_devices(0).unwrap();
    // Note: This test may need adjustment based on timing
}

#[test]
fn test_enforce_user_device_limit() {
    let store = create_test_store();
    let user_id = store.add_user("testuser", "test@example.com").unwrap();

    // Register 5 devices
    for i in 0..5 {
        let reg = DeviceRegistration {
            device_uuid: format!("uuid-{}", i),
            device_type: DeviceType::Android,
            device_name: None,
            os_info: None,
        };
        let device_id = store.register_or_update_device(&reg).unwrap();
        store.associate_device_with_user(device_id, user_id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different last_seen
    }

    // Enforce limit of 3
    let deleted = store.enforce_user_device_limit(user_id, 3).unwrap();
    assert_eq!(deleted, 2);

    let remaining = store.get_user_devices(user_id).unwrap();
    assert_eq!(remaining.len(), 3);
}
```

---

### [ ] Task 10.4: Add AuthToken with device tests
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
#[test]
fn test_auth_token_with_device_id() {
    let store = create_test_store();
    let user_id = store.add_user("testuser", "test@example.com").unwrap();

    let reg = DeviceRegistration {
        device_uuid: "token-test-uuid".to_string(),
        device_type: DeviceType::Web,
        device_name: None,
        os_info: None,
    };
    let device_id = store.register_or_update_device(&reg).unwrap();

    let token = AuthToken {
        user_id,
        value: AuthTokenValue::generate(),
        created: SystemTime::now(),
        last_used: None,
        device_id,
    };

    store.add_user_auth_token(token.clone()).unwrap();

    let retrieved = store.get_user_auth_token(token.value.value()).unwrap().unwrap();
    assert_eq!(retrieved.device_id, device_id);
}

#[test]
fn test_token_cascade_delete_on_device_delete() {
    let store = create_test_store();
    let user_id = store.add_user("testuser", "test@example.com").unwrap();

    let reg = DeviceRegistration {
        device_uuid: "cascade-test-uuid".to_string(),
        device_type: DeviceType::Web,
        device_name: None,
        os_info: None,
    };
    let device_id = store.register_or_update_device(&reg).unwrap();

    let token = AuthToken {
        user_id,
        value: AuthTokenValue::generate(),
        created: SystemTime::now(),
        last_used: None,
        device_id,
    };
    let token_value = token.value.value().to_string();
    store.add_user_auth_token(token).unwrap();

    // Delete device via enforce_user_device_limit (set limit to 0)
    store.enforce_user_device_limit(user_id, 0).unwrap();

    // Token should be gone
    let retrieved = store.get_user_auth_token(&token_value).unwrap();
    assert!(retrieved.is_none());
}
```

---

### [ ] Task 10.5: Add integration tests for login flow
**File**: Create new test file or add to existing integration tests

**Implementation**:
Integration tests should cover:
1. Login with valid credentials and valid device info - success
2. Login with valid credentials but missing device_uuid - 400 error
3. Login with invalid device_uuid format - 400 error
4. Multiple logins from same device reuse device record
5. Device persists across logout/login cycle

---

### [ ] Task 10.6: Add migration tests
**File**: `catalog-server/src/user/sqlite_user_store.rs`

**Implementation**:
```rust
#[test]
fn test_migration_v7_to_v8() {
    // Create a V7 database
    // Run migration to V8
    // Verify device table exists
    // Verify auth_token has device_id column
    // Verify old tokens were deleted
}
```

---

## Phase 11: Final Cleanup and Verification

### [ ] Task 11.1: Run cargo check
Verify all code compiles without errors:
```bash
cd catalog-server && cargo check
```

---

### [ ] Task 11.2: Run cargo clippy
Fix any linting issues:
```bash
cd catalog-server && cargo clippy
```

---

### [ ] Task 11.3: Run all tests
```bash
cd catalog-server && cargo test
```

---

### [ ] Task 11.4: Test migration manually
1. Create a V7 database with existing tokens
2. Run the server to trigger migration
3. Verify schema changes applied correctly
4. Verify tokens were deleted

---

### [ ] Task 11.5: Test login flow end-to-end
1. Start server with fresh database
2. Create a user
3. Login with device info
4. Verify device record created
5. Verify session contains device info
6. Logout and login again - verify same device reused

---

## Summary of Files to Modify

| File | Changes |
|------|---------|
| `catalog-server/src/user/device.rs` | **NEW** - DeviceType, Device, DeviceRegistration, validation |
| `catalog-server/src/user/mod.rs` | Export device module |
| `catalog-server/src/user/auth.rs` | Add device_id to AuthToken |
| `catalog-server/src/user/user_store.rs` | Add DeviceStore trait, update FullUserStore |
| `catalog-server/src/user/sqlite_user_store.rs` | Migration V8, implement DeviceStore, update token methods |
| `catalog-server/src/user/user_manager.rs` | Add device methods, update generate_auth_token |
| `catalog-server/src/server/server.rs` | Update LoginBody, login handler, error responses |
| `catalog-server/src/server/session.rs` | Add device info to Session |

---

## Dependency Graph

```
Phase 1 (device.rs types)
    ↓
Phase 2 (DeviceStore trait)
    ↓
Phase 3 (AuthToken changes)
    ↓
Phase 4 (SQLite schema/migration)
    ↓
Phase 5 (DeviceStore implementation)
    ↓
Phase 6 (Auth token persistence updates)
    ↓
Phase 7 (UserManager changes)
    ↓
Phase 8 (Server changes)
    ↓
Phase 9 (Session changes)
    ↓
Phase 10 (Testing)
    ↓
Phase 11 (Verification)
```

Each phase depends on the previous phases being complete. Within a phase, tasks can often be done in parallel.
