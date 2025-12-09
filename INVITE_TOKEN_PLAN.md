# Invite Token Feature Implementation Plan

One-time login links for existing users. Admin creates a time-limited token tied to a user, shares the link, recipient clicks and gets logged in automatically. Token is invalidated after use.

## Design Decisions

- **For existing users only** - token tied to existing user account
- **Default expiration**: 4 hours
- **Multiple tokens per user**: Allowed
- **Token storage**: Plain text (like auth_token table) to allow retrieval
- **Expired/used tokens**: Kept for audit purposes
- **Required permission**: `ManagePermissions`

---

## 1. Database Schema

New table `invite_token` in user.db:

```sql
CREATE TABLE invite_token (
    id INTEGER PRIMARY KEY,
    token TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    created_by INTEGER NOT NULL,
    created INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    expires INTEGER NOT NULL,
    used_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES user(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES user(id) ON DELETE CASCADE
);
CREATE INDEX idx_invite_token_user_id ON invite_token(user_id);
CREATE INDEX idx_invite_token_token ON invite_token(token);
```

**File**: `src/user/sqlite_user_store.rs` - Add table definition constant

---

## 2. Data Types

**File**: `src/user/auth.rs` (or new file `src/user/invite_token.rs`)

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

// For API responses (excludes raw token in list views if needed)
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

## 3. UserStore Trait Methods

**File**: `src/user/user_store.rs`

Add to `UserStore` trait:

```rust
// Invite tokens
fn create_invite_token(
    &mut self,
    token: &str,
    user_id: usize,
    created_by: usize,
    expires: SystemTime,
) -> Result<usize>;

fn get_invite_token_by_id(&self, token_id: usize) -> Result<Option<InviteToken>>;

fn get_invite_token_by_value(&self, token: &str) -> Result<Option<InviteToken>>;

fn mark_invite_token_used(&mut self, token_id: usize) -> Result<()>;

fn delete_invite_token(&mut self, token_id: usize) -> Result<()>;

fn list_all_invite_tokens(&self) -> Result<Vec<InviteToken>>;

fn list_invite_tokens_for_user(&self, user_id: usize) -> Result<Vec<InviteToken>>;
```

---

## 4. SqliteUserStore Implementation

**File**: `src/user/sqlite_user_store.rs`

Implement all trait methods with standard SQLite operations:
- INSERT for create
- SELECT for get/list
- UPDATE for mark_used
- DELETE for delete

Token generation (32 bytes, base64url):
```rust
fn generate_invite_token_value() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}
```

---

## 5. UserManager Methods

**File**: `src/user/user_manager.rs`

```rust
pub fn create_invite_token(
    &mut self,
    user_handle: &str,
    created_by: usize,
    expires_in_secs: Option<u64>,
) -> Result<InviteToken>

pub fn redeem_invite_token(
    &mut self,
    token: &str,
    device_registration: DeviceRegistration,
) -> Result<(AuthToken, usize)>  // Returns (auth_token, user_id)

pub fn revoke_invite_token(&mut self, token_id: usize) -> Result<()>

pub fn list_all_invite_tokens(&self) -> Result<Vec<InviteTokenView>>

pub fn list_invite_tokens_for_user(&self, user_handle: &str) -> Result<Vec<InviteTokenView>>

pub fn get_invite_token(&self, token_id: usize) -> Result<Option<InviteTokenView>>
```

**Redeem logic**:
1. Look up token by value
2. Check token exists, not expired, not used
3. Get user_id from token
4. Register/update device (same as login)
5. Generate auth token for user
6. Mark invite token as used
7. Return auth token

---

## 6. API Endpoints

**File**: `src/server/server.rs`

### Admin Routes (require ManagePermissions)

```
POST   /v1/admin/users/{handle}/invite-token
       Body: { "expires_in_hours": 4 }  // optional, default 4
       Response: {
         "id": 123,
         "token": "abc...",
         "user_handle": "john",
         "expires_at": 1234567890,
         "link": "pezzottify://invite?server=https://...&token=abc..."
       }

GET    /v1/admin/invite-tokens
       Response: [{ id, token, user_id, user_handle, created_by, created_by_handle, created, expires, used_at }, ...]

GET    /v1/admin/users/{handle}/invite-tokens
       Response: [{ id, token, user_id, user_handle, created_by, created_by_handle, created, expires, used_at }, ...]

GET    /v1/admin/invite-tokens/{id}
       Response: { id, token, user_id, user_handle, created_by, created_by_handle, created, expires, used_at }

DELETE /v1/admin/invite-tokens/{id}
       Response: 204 No Content
```

### Auth Route (no authentication required)

```
POST   /v1/auth/redeem-invite
       Body: {
         "token": "abc...",
         "device_uuid": "...",
         "device_type": "android|web|ios",
         "device_name": "Pixel 7",
         "os_info": "Android 14"  // optional
       }
       Response: Same as login response (auth token in cookie + JSON)
```

---

## 7. Request/Response Types

**File**: `src/server/server.rs` (or separate types file)

```rust
#[derive(Deserialize)]
struct CreateInviteTokenBody {
    expires_in_hours: Option<u64>,  // Default: 4
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

## 8. Deep Link Format

```
pezzottify://invite?server=https://example.com&token=<token>
```

The server needs to know its own URL to generate the link. Options:
- Config option `--public-url`
- Or construct from request headers (Host header)
- Or omit server from link if single-server deployment

For now, use request's Host header to construct the link.

---

## 9. Implementation Order

1. **Schema**: Add table definition, bump schema version
2. **Types**: Add `InviteToken` struct
3. **UserStore trait**: Add method signatures
4. **SqliteUserStore**: Implement methods
5. **UserManager**: Add high-level methods
6. **Server routes**: Add endpoints
7. **Tests**: Unit tests for store, integration tests for endpoints

---

## 10. Files to Modify

| File | Changes |
|------|---------|
| `src/user/sqlite_user_store.rs` | Table definition, trait impl |
| `src/user/user_store.rs` | Trait method signatures |
| `src/user/user_manager.rs` | High-level API |
| `src/user/auth.rs` | `InviteToken` type (or new file) |
| `src/user/mod.rs` | Export new types |
| `src/server/server.rs` | Route handlers, request/response types |

---

## 11. Error Cases

- Token not found → 404
- Token expired → 410 Gone (or 401 with message)
- Token already used → 410 Gone (or 401 with message)
- User deleted (cascade should delete token, but check) → 404
- Invalid device registration → 400

---

## 12. Future Considerations (not in initial implementation)

- Rate limiting on redeem endpoint
- Audit log entries for token creation/use
- WebSocket notification when token is used
- Web UI for admin to manage tokens
- Android deep link handler
