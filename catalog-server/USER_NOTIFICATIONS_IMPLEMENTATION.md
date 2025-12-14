# User Notifications - Detailed Implementation Plan

This document breaks down the implementation into small, sequential tasks. Each task should be completable in a single coding session.

---

## Phase 1: Core Infrastructure

### Task 1.1: Create Notification Models
**Status:** [x]

**Goal:** Define the core data models for notifications.

**File to create:** `src/notifications/models.rs`

**Context:** Follow the pattern used in `src/user/user_models.rs` for data structures.

**Implementation:**
```rust
use serde::{Deserialize, Serialize};

/// Notification type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    DownloadCompleted,
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}

/// A user notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: Option<String>,
    pub data: serde_json::Value,
    pub read_at: Option<i64>,
    pub created_at: i64,
}

/// Data payload for DownloadCompleted notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadCompletedData {
    pub album_id: String,
    pub album_name: String,
    pub artist_name: String,
    pub image_id: Option<String>,
    pub request_id: String,
}
```

**Tests:** Unit tests for serialization/deserialization in `models.rs`.

---

### Task 1.2: Create Notification Module Structure
**Status:** [x]

**Goal:** Set up the module structure for notifications.

**Files to create:**
- `src/notifications/mod.rs`

**File to modify:**
- `src/lib.rs` - add `pub mod notifications;`

**Content of `mod.rs`:**
```rust
mod models;
mod store;

pub use models::{Notification, NotificationType, DownloadCompletedData};
pub use store::NotificationStore;
```

---

### Task 1.3: Add Notification Sync Events
**Status:** [x]

**Goal:** Add `NotificationCreated` and `NotificationRead` to the `UserEvent` enum.

**File to modify:** `src/user/sync_events.rs`

**Changes:**
1. Import `Notification` from notifications module
2. Add two new variants to `UserEvent`:

```rust
use crate::notifications::Notification;

// In UserEvent enum, add:
#[serde(rename = "notification_created")]
NotificationCreated {
    notification: Notification,
},

#[serde(rename = "notification_read")]
NotificationRead {
    notification_id: String,
    read_at: i64,
},
```

3. Update `event_type()` method:
```rust
UserEvent::NotificationCreated { .. } => "notification_created",
UserEvent::NotificationRead { .. } => "notification_read",
```

4. Add serialization tests for both new event types.

---

### Task 1.4: Define NotificationStore Trait
**Status:** [x]

**Goal:** Define the trait interface for notification storage.

**File to create:** `src/notifications/store.rs`

**Context:** Follow the pattern from `src/user/user_store.rs`.

**Implementation:**
```rust
use anyhow::Result;
use super::models::{Notification, NotificationType};

/// Trait for notification storage operations
pub trait NotificationStore: Send + Sync {
    /// Create a notification for a user.
    /// Enforces the 100-notification-per-user limit by deleting oldest if needed.
    /// Returns the created notification with its ID and timestamps set.
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<Notification>;

    /// Get all notifications for a user, ordered by created_at DESC.
    fn get_user_notifications(&self, user_id: usize) -> Result<Vec<Notification>>;

    /// Get a single notification by ID (verifies ownership).
    fn get_notification(&self, notification_id: &str, user_id: usize) -> Result<Option<Notification>>;

    /// Mark a notification as read. Returns the updated notification.
    /// Returns None if notification doesn't exist or doesn't belong to user.
    fn mark_notification_read(&self, notification_id: &str, user_id: usize) -> Result<Option<Notification>>;

    /// Get count of unread notifications for a user.
    fn get_unread_count(&self, user_id: usize) -> Result<usize>;
}
```

---

### Task 1.5: Add Database Table Schema
**Status:** [ ]

**Goal:** Add the `user_notifications` table to the user database schema.

**File to modify:** `src/user/sqlite_user_store.rs`

**Changes:**
1. Add table definition constant (follow pattern of `LIKED_CONTENT_TABLE_V_2`):

```rust
const USER_NOTIFICATIONS_TABLE_V_1: Table = Table {
    name: "user_notifications",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true, is_unique = true),
        sqlite_column!("user_id", &SqlType::Integer, non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "user",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("notification_type", &SqlType::Text, non_null = true),
        sqlite_column!("title", &SqlType::Text, non_null = true),
        sqlite_column!("body", &SqlType::Text),
        sqlite_column!("data", &SqlType::Text, non_null = true),  // JSON
        sqlite_column!("read_at", &SqlType::Integer),  // NULL = unread
        sqlite_column!("created_at", &SqlType::Integer, non_null = true,
            default_value = Some(DEFAULT_TIMESTAMP)),
    ],
    unique_constraints: &[],
    indices: &[("idx_notifications_user_created", "user_id, created_at DESC")],
};
```

2. Add to `USER_DB_TABLES` array
3. Increment schema version

---

### Task 1.6: Implement SqliteNotificationStore
**Status:** [ ]

**Goal:** Implement `NotificationStore` trait for SQLite.

**File to modify:** `src/user/sqlite_user_store.rs`

**Implementation notes:**
- Add `impl NotificationStore for SqliteUserStore`
- `create_notification`:
  1. Generate UUID for notification ID
  2. Get current timestamp
  3. INSERT the notification
  4. DELETE oldest notifications if user has > 100
  5. Return the created notification

```rust
impl NotificationStore for SqliteUserStore {
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<Notification> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp();
        let type_str = serde_json::to_string(&notification_type)?;
        let data_str = serde_json::to_string(&data)?;

        let conn = self.connection.lock().unwrap();

        // Insert the notification
        conn.execute(
            "INSERT INTO user_notifications (id, user_id, notification_type, title, body, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, user_id, type_str, title, body, data_str, created_at],
        )?;

        // Enforce 100-per-user limit: delete oldest beyond limit
        conn.execute(
            "DELETE FROM user_notifications WHERE user_id = ?1 AND id NOT IN (
                SELECT id FROM user_notifications WHERE user_id = ?1
                ORDER BY created_at DESC LIMIT 100
            )",
            params![user_id],
        )?;

        Ok(Notification {
            id,
            notification_type,
            title,
            body,
            data,
            read_at: None,
            created_at,
        })
    }

    // ... implement other methods
}
```

**Tests:** Add integration tests for:
- Creating notifications
- 100-limit enforcement
- Marking as read
- Getting notifications for user

---

### Task 1.7: Export NotificationStore from user module
**Status:** [ ]

**Goal:** Make `NotificationStore` accessible and integrate with `FullUserStore`.

**Files to modify:**
- `src/user/mod.rs` - re-export `NotificationStore`
- `src/user/user_store.rs` - add `NotificationStore` to `FullUserStore` trait bounds
- `src/notifications/mod.rs` - re-export from user module instead

**Decision:** The `NotificationStore` implementation lives in `sqlite_user_store.rs` since it uses the same database. The `notifications` module only contains models.

**Changes to `src/user/user_store.rs`:**
```rust
use crate::notifications::NotificationStore;

pub trait FullUserStore:
    UserStore
    + UserBandwidthStore
    + UserListeningStore
    + UserSettingsStore
    + DeviceStore
    + UserEventStore
    + NotificationStore  // ADD THIS
{
}
```

**Changes to `src/user/mod.rs`:**
```rust
pub use crate::notifications::{Notification, NotificationStore, NotificationType};
```

---

### Task 1.8: Extend SyncStateResponse with Notifications
**Status:** [ ]

**Goal:** Add notifications to the sync state response.

**File to modify:** `src/server/server.rs`

**Changes:**
1. Add `notifications` field to `SyncStateResponse`:

```rust
#[derive(Serialize)]
struct SyncStateResponse {
    seq: i64,
    likes: LikesState,
    settings: Vec<UserSetting>,
    playlists: Vec<PlaylistState>,
    permissions: Vec<Permission>,
    notifications: Vec<Notification>,  // ADD THIS
}
```

2. Update `get_sync_state` handler to fetch and include notifications:

```rust
// After getting permissions, add:
let notifications = match um.get_user_notifications(session.user_id) {
    Ok(n) => n,
    Err(err) => {
        error!("Error getting user notifications: {}", err);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
};

// Include in response:
Json(SyncStateResponse {
    seq,
    likes: LikesState { albums, artists, tracks },
    settings,
    playlists,
    permissions,
    notifications,  // ADD THIS
})
```

---

### Task 1.9: Add Mark-as-Read Endpoint
**Status:** [ ]

**Goal:** Implement `POST /v1/user/notifications/{id}/read` endpoint.

**File to modify:** `src/server/server.rs`

**Implementation:**
1. Add handler function:

```rust
/// POST /v1/user/notifications/{id}/read - Mark notification as read
async fn mark_notification_read(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(notification_id): Path<String>,
) -> Response {
    let (notification, stored_event) = {
        let um = user_manager.lock().unwrap();

        // Mark as read
        let notification = match um.mark_notification_read(&notification_id, session.user_id) {
            Ok(Some(n)) => n,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error marking notification read: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Log sync event
        let event = UserEvent::NotificationRead {
            notification_id: notification_id.clone(),
            read_at: notification.read_at.unwrap(),
        };

        let stored_event = match um.append_event(session.user_id, &event) {
            Ok(e) => Some(e),
            Err(err) => {
                warn!("Failed to log notification_read event: {}", err);
                None
            }
        };

        (notification, stored_event)
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = ServerMessage::new(
            SYNC,
            SyncEventMessage { event: stored_event },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}
```

2. Add route:

```rust
// In user_routes section, add:
.route("/notifications/{id}/read", post(mark_notification_read))
```

---

### Task 1.10: Unit Tests for Core Infrastructure
**Status:** [ ]

**Goal:** Add comprehensive tests for all Phase 1 components.

**Files to modify/create:**
- Tests in `src/notifications/models.rs`
- Tests in `src/user/sqlite_user_store.rs`

**Test cases:**
1. Notification model serialization/deserialization
2. NotificationType serialization
3. DownloadCompletedData serialization
4. Creating notifications
5. 100-per-user limit enforcement
6. Marking notifications as read
7. Getting user notifications (ordered by created_at DESC)
8. UserEvent::NotificationCreated serialization
9. UserEvent::NotificationRead serialization

---

## Phase 2: Download Manager Integration

### Task 2.1: Create NotificationService
**Status:** [ ]

**Goal:** Create a service that creates notifications and handles sync event emission + WebSocket broadcast.

**File to create:** `src/notifications/service.rs`

**Context:** Follow the pattern from `src/download_manager/sync_notifier.rs`.

**Implementation:**
```rust
use std::sync::Arc;
use tracing::{debug, warn};

use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::msg_types::SYNC;
use crate::server::websocket::messages::sync::SyncEventMessage;
use crate::server::websocket::messages::ServerMessage;
use crate::user::sync_events::UserEvent;
use crate::user::FullUserStore;

use super::models::{Notification, NotificationType};

pub struct NotificationService {
    user_store: Arc<dyn FullUserStore>,
    connection_manager: Arc<ConnectionManager>,
}

impl NotificationService {
    pub fn new(
        user_store: Arc<dyn FullUserStore>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            user_store,
            connection_manager,
        }
    }

    /// Create a notification and broadcast to all user's devices.
    pub async fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> anyhow::Result<Notification> {
        // 1. Create notification in database
        let notification = self.user_store.create_notification(
            user_id,
            notification_type,
            title,
            body,
            data,
        )?;

        // 2. Log sync event
        let event = UserEvent::NotificationCreated {
            notification: notification.clone(),
        };

        let stored_event = match self.user_store.append_event(user_id, &event) {
            Ok(e) => e,
            Err(err) => {
                warn!("Failed to log notification_created event: {}", err);
                return Ok(notification);
            }
        };

        // 3. Broadcast to all user's devices
        let ws_msg = ServerMessage::new(
            SYNC,
            SyncEventMessage { event: stored_event },
        );

        let failed = self
            .connection_manager
            .broadcast_to_user(user_id, ws_msg)
            .await;

        if !failed.is_empty() {
            debug!(
                "Failed to send notification to {} devices for user {}",
                failed.len(),
                user_id
            );
        }

        Ok(notification)
    }
}
```

**Update `src/notifications/mod.rs`:**
```rust
mod models;
mod service;

pub use models::{DownloadCompletedData, Notification, NotificationType};
pub use service::NotificationService;
```

---

### Task 2.2: Integrate NotificationService with DownloadManager
**Status:** [ ]

**Goal:** Add `NotificationService` to `DownloadManager` and call it on download completion.

**File to modify:** `src/download_manager/manager.rs`

**Changes:**
1. Add `NotificationService` field to `DownloadManager`:

```rust
use crate::notifications::NotificationService;
use tokio::sync::RwLock;

pub struct DownloadManager {
    // ... existing fields ...
    notification_service: RwLock<Option<Arc<NotificationService>>>,
}
```

2. Add setter method:

```rust
pub async fn set_notification_service(&self, service: Arc<NotificationService>) {
    let mut guard = self.notification_service.write().await;
    *guard = Some(service);
}
```

3. In `process_next` or wherever `notify_completed` is called (around line 1058), add notification creation:

```rust
// After: notifier.notify_completed(parent).await;
// Add:
if let Some(ref notification_service) = *self.notification_service.read().await {
    if let Some(user_id_str) = &parent.requested_by_user_id {
        if let Ok(user_id) = user_id_str.parse::<usize>() {
            let data = serde_json::json!({
                "album_id": parent.content_id,
                "album_name": parent.content_name.clone().unwrap_or_default(),
                "artist_name": parent.artist_name.clone().unwrap_or_default(),
                "image_id": null,  // TODO: Get from catalog if available
                "request_id": parent.id,
            });

            let title = format!(
                "\"{}\" is ready",
                parent.content_name.as_deref().unwrap_or("Album")
            );
            let body = parent.artist_name.as_ref().map(|a| format!("by {}", a));

            if let Err(e) = notification_service.create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                title,
                body,
                data,
            ).await {
                warn!("Failed to create download completion notification: {}", e);
            }
        }
    }
}
```

---

### Task 2.3: Initialize NotificationService in Server Startup
**Status:** [ ]

**Goal:** Create and wire up `NotificationService` during server initialization.

**File to modify:** `src/server/server.rs` (or wherever server setup happens)

**Changes:**
1. After creating `ConnectionManager` and `UserManager`, create `NotificationService`:

```rust
let notification_service = Arc::new(NotificationService::new(
    user_store.clone(),
    connection_manager.clone(),
));
```

2. Pass to `DownloadManager`:

```rust
download_manager.set_notification_service(notification_service.clone()).await;
```

---

### Task 2.4: Get Album Image ID for Notification
**Status:** [ ]

**Goal:** Include `image_id` in download completion notification when available.

**File to modify:** `src/download_manager/manager.rs`

**Context:** When creating the notification, we have `content_id` (album ID). We can query the catalog to get the album's image.

**Implementation:**
```rust
// Get album image if available
let image_id = self.catalog_store
    .get_album(&parent.content_id)
    .ok()
    .flatten()
    .and_then(|album| album.image_id);

let data = serde_json::json!({
    "album_id": parent.content_id,
    "album_name": parent.content_name.clone().unwrap_or_default(),
    "artist_name": parent.artist_name.clone().unwrap_or_default(),
    "image_id": image_id,
    "request_id": parent.id,
});
```

---

### Task 2.5: E2E Tests for Download -> Notification Flow
**Status:** [ ]

**Goal:** Add integration tests verifying the complete flow.

**File to create:** `tests/e2e_notification_tests.rs`

**Test cases:**
1. Request album download -> download completes -> notification created
2. Notification appears in sync state
3. Notification has correct data payload
4. WebSocket receives NotificationCreated event
5. Mark notification as read -> NotificationRead event broadcast

---

## Phase 3: Android Client

### Task 3.1: Add Notification Models to domain module
**Status:** [ ]

**Goal:** Define Kotlin data classes for notifications.

**File to create:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/notifications/NotificationModels.kt`

**Context:** Follow the pattern in `domain/src/main/java/.../sync/SyncEvent.kt`.

**Implementation:**
```kotlin
package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class NotificationType {
    @SerialName("download_completed")
    DownloadCompleted,
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}

@Serializable
data class Notification(
    val id: String,
    @SerialName("notification_type")
    val notificationType: NotificationType,
    val title: String,
    val body: String? = null,
    val data: kotlinx.serialization.json.JsonElement,
    @SerialName("read_at")
    val readAt: Long? = null,
    @SerialName("created_at")
    val createdAt: Long,
)

@Serializable
data class DownloadCompletedData(
    @SerialName("album_id")
    val albumId: String,
    @SerialName("album_name")
    val albumName: String,
    @SerialName("artist_name")
    val artistName: String,
    @SerialName("image_id")
    val imageId: String? = null,
    @SerialName("request_id")
    val requestId: String,
)
```

---

### Task 3.2: Add Notification Sync Events
**Status:** [ ]

**Goal:** Add `NotificationCreated` and `NotificationRead` to the `SyncEvent` sealed interface.

**File to modify:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`

**Changes:**
```kotlin
import com.lelloman.pezzottify.android.domain.notifications.Notification

// Add to SyncEvent sealed interface:

@Serializable
@SerialName("notification_created")
data class NotificationCreated(
    val notification: Notification,
) : SyncEvent

@Serializable
@SerialName("notification_read")
data class NotificationRead(
    @SerialName("notification_id")
    val notificationId: String,
    @SerialName("read_at")
    val readAt: Long,
) : SyncEvent
```

---

### Task 3.3: Update Sync State Response Parsing
**Status:** [ ]

**Goal:** Add notifications to `SyncStateResponse` in the remoteapi module.

**File to modify:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/sync/SyncStateResponse.kt` (or similar)

**Changes:**
```kotlin
@Serializable
data class SyncStateResponse(
    val seq: Long,
    val likes: LikesState,
    val settings: List<UserSettingDto>,
    val playlists: List<PlaylistDto>,
    val permissions: List<String>,
    val notifications: List<Notification>,  // ADD THIS
)
```

**Note:** Verify the actual file location by checking where `SyncStateResponse` is defined.

---

### Task 3.4: Create NotificationRepository Interface
**Status:** [ ]

**Goal:** Define the repository interface for notifications.

**File to create:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/notifications/NotificationRepository.kt`

**Context:** Follow the pattern from `DownloadStatusRepository.kt`.

**Implementation:**
```kotlin
package com.lelloman.pezzottify.android.domain.notifications

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

interface NotificationRepository {
    /** All notifications, ordered by createdAt DESC */
    val notifications: StateFlow<List<Notification>>

    /** Count of unread notifications */
    val unreadCount: StateFlow<Int>

    /** Flow of real-time notification updates */
    fun observeUpdates(): Flow<NotificationUpdate>

    /** Called by SyncManager on full sync */
    suspend fun setNotifications(notifications: List<Notification>)

    /** Called by SyncManager when notification_created event received */
    suspend fun onNotificationCreated(notification: Notification)

    /** Called by SyncManager when notification_read event received */
    suspend fun onNotificationRead(notificationId: String, readAt: Long)

    /** Mark notification as read (triggers API call + local update) */
    suspend fun markAsRead(notificationId: String): Result<Unit>

    /** Clear all notifications (on logout) */
    suspend fun clear()
}

sealed interface NotificationUpdate {
    data class Created(val notification: Notification) : NotificationUpdate
    data class Read(val notificationId: String, val readAt: Long) : NotificationUpdate
}
```

---

### Task 3.5: Implement NotificationRepositoryImpl
**Status:** [ ]

**Goal:** Implement the notification repository with in-memory cache.

**File to create:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/notifications/NotificationRepositoryImpl.kt`

**Context:** Follow pattern from `DownloadStatusRepositoryImpl.kt` - in-memory StateFlow + SharedFlow for updates.

**Implementation:**
```kotlin
@Singleton
class NotificationRepositoryImpl @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) : NotificationRepository {

    private val _notifications = MutableStateFlow<List<Notification>>(emptyList())
    override val notifications: StateFlow<List<Notification>> = _notifications.asStateFlow()

    override val unreadCount: StateFlow<Int> = _notifications
        .map { list -> list.count { it.readAt == null } }
        .stateIn(CoroutineScope(Dispatchers.Default), SharingStarted.Eagerly, 0)

    private val _updates = MutableSharedFlow<NotificationUpdate>(extraBufferCapacity = 64)
    override fun observeUpdates(): Flow<NotificationUpdate> = _updates.asSharedFlow()

    override suspend fun setNotifications(notifications: List<Notification>) {
        _notifications.value = notifications.sortedByDescending { it.createdAt }
    }

    override suspend fun onNotificationCreated(notification: Notification) {
        _notifications.update { current ->
            (listOf(notification) + current).take(100)
        }
        _updates.emit(NotificationUpdate.Created(notification))
    }

    override suspend fun onNotificationRead(notificationId: String, readAt: Long) {
        _notifications.update { current ->
            current.map {
                if (it.id == notificationId) it.copy(readAt = readAt) else it
            }
        }
        _updates.emit(NotificationUpdate.Read(notificationId, readAt))
    }

    override suspend fun markAsRead(notificationId: String): Result<Unit> {
        return runCatching {
            remoteApiClient.markNotificationRead(notificationId)
            // Local update happens via sync event
        }
    }

    override suspend fun clear() {
        _notifications.value = emptyList()
    }
}
```

---

### Task 3.6: Add RemoteApiClient Method for Mark-as-Read
**Status:** [ ]

**Goal:** Add API method to mark notification as read.

**File to modify:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/RemoteApiClient.kt` (or internal implementation)

**Implementation:**
```kotlin
// In RemoteApiClient interface:
suspend fun markNotificationRead(notificationId: String)

// In implementation:
override suspend fun markNotificationRead(notificationId: String) {
    httpClient.post("v1/user/notifications/$notificationId/read")
}
```

---

### Task 3.7: Integrate with SyncManager
**Status:** [ ]

**Goal:** Handle notification events in SyncManager.

**File to modify:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncManagerImpl.kt`

**Changes:**

1. Inject `NotificationRepository`:
```kotlin
@Singleton
class SyncManagerImpl @Inject constructor(
    // ... existing dependencies ...
    private val notificationRepository: NotificationRepository,
) : SyncManager {
```

2. Update `fullSync()` to include notifications:
```kotlin
private suspend fun fullSync(): Boolean {
    val state = remoteApiClient.fetchSyncState()

    // ... existing code for likes, settings, playlists, permissions ...

    // Add notifications
    notificationRepository.setNotifications(state.notifications)

    saveCursor(state.seq)
    return true
}
```

3. Update `applyStoredEvent()` to handle new events:
```kotlin
private suspend fun applyStoredEvent(event: StoredEvent) {
    when (val syncEvent = event.event) {
        // ... existing cases ...

        is SyncEvent.NotificationCreated -> {
            notificationRepository.onNotificationCreated(syncEvent.notification)
        }
        is SyncEvent.NotificationRead -> {
            notificationRepository.onNotificationRead(
                syncEvent.notificationId,
                syncEvent.readAt
            )
        }
    }
}
```

4. Update `cleanup()`:
```kotlin
override suspend fun cleanup() {
    // ... existing cleanup ...
    notificationRepository.clear()
}
```

---

### Task 3.8: Add Hilt Module for NotificationRepository
**Status:** [ ]

**Goal:** Register NotificationRepository in Hilt dependency injection.

**File to modify:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/di/DomainModule.kt` (or similar)

**Implementation:**
```kotlin
@Module
@InstallIn(SingletonComponent::class)
abstract class NotificationModule {
    @Binds
    @Singleton
    abstract fun bindNotificationRepository(
        impl: NotificationRepositoryImpl
    ): NotificationRepository
}
```

---

### Task 3.9: Create NotificationListScreen UI
**Status:** [ ]

**Goal:** Create a Compose screen to display notifications.

**File to create:** `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/notifications/NotificationListScreen.kt`

**Context:** Follow pattern from `LibraryScreen.kt` for list structure.

**Implementation:**
```kotlin
@Composable
fun NotificationListScreen(
    viewModel: NotificationListViewModel = hiltViewModel(),
    onNotificationClick: (Notification) -> Unit,
    onBackClick: () -> Unit,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Notifications") },
                navigationIcon = {
                    IconButton(onClick = onBackClick) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            items(state.notifications, key = { it.id }) { notification ->
                NotificationListItem(
                    notification = notification,
                    onClick = {
                        viewModel.markAsRead(notification.id)
                        onNotificationClick(notification)
                    }
                )
            }

            if (state.notifications.isEmpty()) {
                item {
                    Box(
                        modifier = Modifier.fillMaxWidth().padding(32.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("No notifications")
                    }
                }
            }
        }
    }
}

@Composable
private fun NotificationListItem(
    notification: Notification,
    onClick: () -> Unit,
) {
    val isUnread = notification.readAt == null

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        color = if (isUnread) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.1f)
                else MaterialTheme.colorScheme.surface
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Unread indicator
            if (isUnread) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .background(MaterialTheme.colorScheme.primary, CircleShape)
                )
                Spacer(Modifier.width(12.dp))
            }

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = notification.title,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (isUnread) FontWeight.Bold else FontWeight.Normal
                )
                notification.body?.let { body ->
                    Text(
                        text = body,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Text(
                    text = formatRelativeTime(notification.createdAt),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}
```

---

### Task 3.10: Create NotificationListViewModel
**Status:** [ ]

**Goal:** ViewModel for notification list screen.

**File to create:** `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/notifications/NotificationListViewModel.kt`

**Implementation:**
```kotlin
@HiltViewModel
class NotificationListViewModel @Inject constructor(
    private val notificationRepository: NotificationRepository,
) : ViewModel() {

    data class State(
        val notifications: List<Notification> = emptyList(),
        val isLoading: Boolean = false,
    )

    val state: StateFlow<State> = notificationRepository.notifications
        .map { State(notifications = it) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), State())

    fun markAsRead(notificationId: String) {
        viewModelScope.launch {
            notificationRepository.markAsRead(notificationId)
        }
    }
}
```

---

### Task 3.11: Add Unread Badge to Navigation
**Status:** [ ]

**Goal:** Show unread notification count in app navigation.

**File to modify:** Depends on app navigation structure - likely `MainScreen.kt` or `BottomNavigation.kt`

**Context:** Observe `notificationRepository.unreadCount` and display badge.

**Implementation example:**
```kotlin
@Composable
fun NotificationIconWithBadge(
    unreadCount: Int,
    onClick: () -> Unit,
) {
    IconButton(onClick = onClick) {
        BadgedBox(
            badge = {
                if (unreadCount > 0) {
                    Badge { Text(unreadCount.coerceAtMost(99).toString()) }
                }
            }
        ) {
            Icon(Icons.Default.Notifications, contentDescription = "Notifications")
        }
    }
}

// In top bar or navigation:
val unreadCount by notificationRepository.unreadCount.collectAsStateWithLifecycle()
NotificationIconWithBadge(
    unreadCount = unreadCount,
    onClick = { navController.navigate("notifications") }
)
```

---

### Task 3.12: Add Navigation Route for Notifications
**Status:** [ ]

**Goal:** Add navigation route to reach notification screen.

**File to modify:** `android/app/src/main/java/com/lelloman/pezzottify/android/app/navigation/Navigation.kt` (or similar)

**Changes:**
```kotlin
// Add to NavHost:
composable("notifications") {
    NotificationListScreen(
        onNotificationClick = { notification ->
            // Navigate to album if download_completed
            when (notification.notificationType) {
                NotificationType.DownloadCompleted -> {
                    val data = Json.decodeFromJsonElement<DownloadCompletedData>(notification.data)
                    navController.navigate("album/${data.albumId}")
                }
            }
        },
        onBackClick = { navController.popBackStack() }
    )
}
```

---

### Task 3.13: Optional - Room Persistence for Offline Support
**Status:** [ ]

**Goal:** Persist notifications to Room database for offline access.

**Note:** This is optional if in-memory storage is sufficient. The current implementation (Task 3.5) uses in-memory StateFlow which gets repopulated on each app start via fullSync.

**If needed, create:**
- `android/localdata/src/main/java/.../notifications/NotificationEntity.kt`
- `android/localdata/src/main/java/.../notifications/NotificationDao.kt`
- Update `NotificationRepositoryImpl` to use Room

---

## Phase 4: Web Client

### Task 4.1: Add Notification State to User Store
**Status:** [ ]

**Goal:** Add notification state management to the user store.

**File to modify:** `web/src/store/user.js`

**Changes:**
```javascript
// Add to state:
const notifications = ref([]);

// Add getters:
const getNotifications = computed(() => notifications.value);
const getUnreadCount = computed(() =>
    notifications.value.filter(n => n.read_at === null).length
);

// Add setters:
const setNotifications = (notifs) => {
    notifications.value = notifs.sort((a, b) => b.created_at - a.created_at);
};

// Add event handlers:
const applyNotificationCreated = (notification) => {
    // Add to front, maintain 100 limit
    notifications.value = [notification, ...notifications.value].slice(0, 100);
};

const applyNotificationRead = (notificationId, readAt) => {
    notifications.value = notifications.value.map(n =>
        n.id === notificationId ? { ...n, read_at: readAt } : n
    );
};

// Add API method:
const markNotificationRead = async (notificationId) => {
    await remoteStore.markNotificationRead(notificationId);
    // Local update happens via sync event
};

// Update reset():
const reset = () => {
    // ... existing resets ...
    notifications.value = [];
};

// Export new items:
return {
    // ... existing exports ...
    notifications,
    getNotifications,
    getUnreadCount,
    setNotifications,
    applyNotificationCreated,
    applyNotificationRead,
    markNotificationRead,
};
```

---

### Task 4.2: Add API Method for Mark-as-Read
**Status:** [ ]

**Goal:** Add remote API method to mark notification as read.

**File to modify:** `web/src/store/remote.js`

**Changes:**
```javascript
const markNotificationRead = async (notificationId) => {
    const response = await axios.post(
        `/v1/user/notifications/${notificationId}/read`
    );
    return response.data;
};

// Add to exports:
return {
    // ... existing exports ...
    markNotificationRead,
};
```

---

### Task 4.3: Update Sync Store for Notifications
**Status:** [ ]

**Goal:** Handle notifications in sync state and events.

**File to modify:** `web/src/store/sync.js`

**Changes:**

1. Update `fullSync()`:
```javascript
const fullSync = async () => {
    const state = await remoteStore.fetchSyncState();

    // ... existing code for likes, settings, playlists, permissions ...

    // Add notifications
    userStore.setNotifications(state.notifications || []);

    saveCursor(state.seq);
};
```

2. Update `applyEvent()`:
```javascript
const applyEvent = (event) => {
    const { type, payload } = event;

    switch (type) {
        // ... existing cases ...

        case "notification_created":
            userStore.applyNotificationCreated(payload.notification);
            break;

        case "notification_read":
            userStore.applyNotificationRead(payload.notification_id, payload.read_at);
            break;
    }
};
```

---

### Task 4.4: Create NotificationDropdown Component
**Status:** [ ]

**Goal:** Create a dropdown component for notifications in the header.

**File to create:** `web/src/components/notifications/NotificationDropdown.vue`

**Context:** Follow pattern from `ContextMenu.vue` for dropdown positioning.

**Implementation:**
```vue
<template>
  <div class="notification-dropdown-container">
    <!-- Trigger button with badge -->
    <button
      class="notification-trigger"
      @click="toggleDropdown"
      ref="triggerRef"
    >
      <BellIcon class="icon" />
      <span v-if="unreadCount > 0" class="badge">
        {{ unreadCount > 99 ? '99+' : unreadCount }}
      </span>
    </button>

    <!-- Dropdown panel -->
    <Transition name="fade">
      <div
        v-if="isOpen"
        class="notification-dropdown"
        ref="dropdownRef"
      >
        <div class="dropdown-header">
          <h3>Notifications</h3>
          <button
            v-if="notifications.length > 0"
            @click="markAllRead"
            class="mark-all-read"
          >
            Mark all read
          </button>
        </div>

        <div class="notification-list">
          <div
            v-for="notification in notifications"
            :key="notification.id"
            class="notification-item"
            :class="{ unread: !notification.read_at }"
            @click="handleNotificationClick(notification)"
          >
            <div class="notification-content">
              <span class="notification-title">{{ notification.title }}</span>
              <span v-if="notification.body" class="notification-body">
                {{ notification.body }}
              </span>
              <span class="notification-time">
                {{ formatRelativeTime(notification.created_at) }}
              </span>
            </div>
            <div v-if="!notification.read_at" class="unread-dot"></div>
          </div>

          <div v-if="notifications.length === 0" class="empty-state">
            No notifications
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '@/store/user';
import { BellIcon } from '@/components/icons';

const router = useRouter();
const userStore = useUserStore();

const isOpen = ref(false);
const triggerRef = ref(null);
const dropdownRef = ref(null);

const notifications = computed(() => userStore.getNotifications);
const unreadCount = computed(() => userStore.getUnreadCount);

const toggleDropdown = () => {
  isOpen.value = !isOpen.value;
};

const handleNotificationClick = async (notification) => {
  // Mark as read
  if (!notification.read_at) {
    await userStore.markNotificationRead(notification.id);
  }

  // Navigate based on notification type
  if (notification.notification_type === 'download_completed') {
    const data = notification.data;
    router.push(`/album/${data.album_id}`);
  }

  isOpen.value = false;
};

const markAllRead = async () => {
  for (const notification of notifications.value) {
    if (!notification.read_at) {
      await userStore.markNotificationRead(notification.id);
    }
  }
};

const formatRelativeTime = (timestamp) => {
  const now = Date.now() / 1000;
  const diff = now - timestamp;

  if (diff < 60) return 'Just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(timestamp * 1000).toLocaleDateString();
};

// Click outside to close
const handleClickOutside = (event) => {
  if (
    dropdownRef.value &&
    !dropdownRef.value.contains(event.target) &&
    !triggerRef.value.contains(event.target)
  ) {
    isOpen.value = false;
  }
};

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});
</script>

<style scoped>
.notification-dropdown-container {
  position: relative;
}

.notification-trigger {
  position: relative;
  background: none;
  border: none;
  cursor: pointer;
  padding: 8px;
}

.badge {
  position: absolute;
  top: 0;
  right: 0;
  background: var(--color-primary);
  color: white;
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 10px;
  min-width: 18px;
  text-align: center;
}

.notification-dropdown {
  position: absolute;
  top: 100%;
  right: 0;
  width: 320px;
  max-height: 400px;
  background: var(--color-background);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  z-index: 1000;
  overflow: hidden;
}

.dropdown-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.dropdown-header h3 {
  margin: 0;
  font-size: 14px;
}

.mark-all-read {
  background: none;
  border: none;
  color: var(--color-primary);
  cursor: pointer;
  font-size: 12px;
}

.notification-list {
  overflow-y: auto;
  max-height: 340px;
}

.notification-item {
  display: flex;
  align-items: flex-start;
  padding: 12px 16px;
  cursor: pointer;
  border-bottom: 1px solid var(--color-border);
}

.notification-item:hover {
  background: var(--color-hover);
}

.notification-item.unread {
  background: var(--color-primary-light);
}

.notification-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.notification-title {
  font-weight: 500;
  font-size: 14px;
}

.notification-body {
  font-size: 12px;
  color: var(--color-text-secondary);
}

.notification-time {
  font-size: 11px;
  color: var(--color-text-muted);
}

.unread-dot {
  width: 8px;
  height: 8px;
  background: var(--color-primary);
  border-radius: 50%;
  margin-left: 8px;
  margin-top: 4px;
}

.empty-state {
  padding: 32px;
  text-align: center;
  color: var(--color-text-muted);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
```

---

### Task 4.5: Add NotificationDropdown to Header
**Status:** [ ]

**Goal:** Integrate notification dropdown into the app header.

**File to modify:** `web/src/components/common/Header.vue` (or `AppHeader.vue`, `TopBar.vue` - check actual filename)

**Changes:**
```vue
<script setup>
import NotificationDropdown from '@/components/notifications/NotificationDropdown.vue';
</script>

<template>
  <!-- In the header actions area, add: -->
  <NotificationDropdown />
</template>
```

---

### Task 4.6: Create BellIcon Component
**Status:** [ ]

**Goal:** Add bell icon for notifications.

**File to create:** `web/src/components/icons/BellIcon.vue`

**Implementation:**
```vue
<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
    <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
  </svg>
</template>
```

**Also update:** `web/src/components/icons/index.js` to export `BellIcon`.

---

### Task 4.7: Optional - Notification Toast on New Notification
**Status:** [ ]

**Goal:** Show a toast notification when a new notification arrives via WebSocket.

**Context:** The app may have a toast/snackbar system. If so, integrate.

**File to modify:** `web/src/store/sync.js` or create a notification toast composable.

**Implementation sketch:**
```javascript
// In applyEvent(), when notification_created:
case "notification_created":
    userStore.applyNotificationCreated(payload.notification);
    // Show toast
    showToast({
        title: payload.notification.title,
        body: payload.notification.body,
        onClick: () => router.push(`/album/${payload.notification.data.album_id}`)
    });
    break;
```

---

### Task 4.8: Add Mark-All-Read API Method (Optional)
**Status:** [ ]

**Goal:** If server supports mark-all-read, add client support.

**Note:** The current server plan doesn't include a mark-all-read endpoint. This task is for if/when it's added.

**Server endpoint needed:** `POST /v1/user/notifications/read-all`

**Client implementation:**
```javascript
// In remote.js:
const markAllNotificationsRead = async () => {
    await axios.post('/v1/user/notifications/read-all');
};

// In user.js:
const markAllNotificationsRead = async () => {
    await remoteStore.markAllNotificationsRead();
    // Local updates happen via sync events
};
```

---

## Implementation Order

### Server (Phase 1 & 2) - Do First
1. **Task 1.1** - Models (no dependencies)
2. **Task 1.2** - Module structure (depends on 1.1)
3. **Task 1.4** - Store trait (depends on 1.1)
4. **Task 1.5** - Database schema (no dependencies)
5. **Task 1.6** - SQLite implementation (depends on 1.4, 1.5)
6. **Task 1.3** - Sync events (depends on 1.1)
7. **Task 1.7** - Export and integrate (depends on 1.6)
8. **Task 1.8** - Sync state response (depends on 1.7)
9. **Task 1.9** - Mark-as-read endpoint (depends on 1.7, 1.3)
10. **Task 1.10** - Tests (depends on all Phase 1)
11. **Task 2.1** - NotificationService (depends on Phase 1)
12. **Task 2.2** - DownloadManager integration (depends on 2.1)
13. **Task 2.3** - Server startup wiring (depends on 2.1)
14. **Task 2.4** - Album image (optional enhancement)
15. **Task 2.5** - E2E tests (depends on all Phase 2)

### Android Client (Phase 3) - Can start after Task 1.8
1. **Task 3.1** - Notification models
2. **Task 3.2** - Sync events
3. **Task 3.3** - Sync state response parsing
4. **Task 3.4** - NotificationRepository interface
5. **Task 3.5** - NotificationRepositoryImpl
6. **Task 3.6** - RemoteApiClient mark-as-read
7. **Task 3.7** - SyncManager integration
8. **Task 3.8** - Hilt module
9. **Task 3.9** - NotificationListScreen UI
10. **Task 3.10** - NotificationListViewModel
11. **Task 3.11** - Unread badge in navigation
12. **Task 3.12** - Navigation route
13. **Task 3.13** - (Optional) Room persistence

### Web Client (Phase 4) - Can start after Task 1.8
1. **Task 4.1** - User store notification state
2. **Task 4.2** - Remote API mark-as-read
3. **Task 4.3** - Sync store integration
4. **Task 4.4** - NotificationDropdown component
5. **Task 4.5** - Header integration
6. **Task 4.6** - BellIcon component
7. **Task 4.7** - (Optional) Toast notifications
8. **Task 4.8** - (Optional) Mark-all-read

---

## File Summary

### Server (catalog-server/)

**New files:**
- `src/notifications/mod.rs`
- `src/notifications/models.rs`
- `src/notifications/service.rs`
- `tests/e2e_notification_tests.rs`

**Modified files:**
- `src/lib.rs` - add `pub mod notifications`
- `src/user/mod.rs` - re-exports
- `src/user/user_store.rs` - add `NotificationStore` to `FullUserStore`
- `src/user/sqlite_user_store.rs` - table schema + implementation
- `src/user/sync_events.rs` - new event types
- `src/server/server.rs` - sync state + mark-read endpoint
- `src/download_manager/manager.rs` - notification creation on completion

### Android (android/)

**New files:**
- `domain/src/main/java/.../notifications/NotificationModels.kt`
- `domain/src/main/java/.../notifications/NotificationRepository.kt`
- `domain/src/main/java/.../notifications/NotificationRepositoryImpl.kt`
- `ui/src/main/java/.../screen/notifications/NotificationListScreen.kt`
- `ui/src/main/java/.../screen/notifications/NotificationListViewModel.kt`

**Modified files:**
- `domain/src/main/java/.../sync/SyncEvent.kt` - new event types
- `domain/src/main/java/.../sync/SyncManagerImpl.kt` - handle notification events
- `domain/src/main/java/.../di/DomainModule.kt` - Hilt bindings
- `remoteapi/src/main/java/.../RemoteApiClient.kt` - mark-as-read method
- `remoteapi/src/main/java/.../sync/SyncStateResponse.kt` - add notifications
- `app/src/main/java/.../navigation/Navigation.kt` - notification route
- Main screen/navigation - unread badge

### Web (web/)

**New files:**
- `src/components/notifications/NotificationDropdown.vue`
- `src/components/icons/BellIcon.vue`

**Modified files:**
- `src/store/user.js` - notification state + methods
- `src/store/remote.js` - mark-as-read API
- `src/store/sync.js` - handle notification events in fullSync + applyEvent
- `src/components/icons/index.js` - export BellIcon
- Header component - add NotificationDropdown

---

## Task Count Summary

| Phase | Tasks | Optional |
|-------|-------|----------|
| Phase 1: Server Core | 10 | 0 |
| Phase 2: Download Integration | 5 | 1 |
| Phase 3: Android | 12 | 1 |
| Phase 4: Web | 6 | 2 |
| **Total** | **33** | **4** |
