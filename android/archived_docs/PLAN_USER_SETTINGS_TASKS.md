# User Settings Implementation Task List

This task list breaks down the implementation of the sync-aware user settings feature as specified in `PLAN_USER_SETTINGS.md`.

**Legend:**
- `[ ]` Pending
- `[~]` In progress
- `[x]` Completed

---

## Phase 1: Domain Layer - Sync Models & Interfaces

### 1.1 Create Permission Enum
- [x] **Create `Permission.kt` in domain/auth**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/Permission.kt`
  - Define enum with values: `AccessCatalog`, `LikeContent`, `OwnPlaylists`, `EditCatalog`, `ManagePermissions`, `IssueContentDownload`, `ServerAdmin`, `ViewAnalytics`
  - Add companion object with `fromSnakeCase(value: String): Permission?` function to parse server format (e.g., "access_catalog" → AccessCatalog)
  - Use `split("_")` and `joinToString` with capitalization to convert snake_case to PascalCase

### 1.2 Create Sync Event Types
- [x] **Create `SyncEvent.kt` in domain/sync**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`
  - Create `SyncEventEnvelope` data class with `@Serializable` annotation containing: `seq: Long`, `type: String`, `payload: JsonElement`, `timestamp: Long`
  - Create `SyncEvent` sealed interface with common property `val seq: Long`
  - Add subclass: `SettingChanged(seq, key: String, value: JsonElement)`
  - Add subclass: `ContentLiked(seq, contentType: String, contentId: String)`
  - Add subclass: `ContentUnliked(seq, contentType: String, contentId: String)`
  - Add subclass: `PermissionGranted(seq, permission: String)`
  - Add subclass: `PermissionRevoked(seq, permission: String)`
  - Add subclass: `PermissionsReset(seq, permissions: List<String>)`
  - Add playlist event subclasses: `PlaylistCreated`, `PlaylistRenamed`, `PlaylistDeleted`, `PlaylistTrackAdded`, `PlaylistTrackRemoved`, `PlaylistTracksReordered`
  - Add fallback subclass: `Unknown(seq, type: String)` for unrecognized event types

### 1.3 Create Sync State Model
- [x] **Create `SyncState.kt` in domain/sync**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncState.kt`
  - Create `SyncState` data class with: `seq: Long`, `likes: LikedContent`, `settings: List<UserSettingDto>`, `playlists: List<PlaylistDto>`, `permissions: Set<Permission>`
  - Create `LikedContent` data class with: `albums: List<String>`, `artists: List<String>`, `tracks: List<String>`
  - Create `PlaylistDto` data class with: `id: String`, `name: String`, `tracks: List<String>`
  - Create `UserSettingDto` data class with: `key: String`, `value: Any`

### 1.4 Create SyncStateStore Interface
- [x] **Create `SyncStateStore.kt` in domain/sync**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncStateStore.kt`
  - Define `SyncStateStore` interface
  - Add cursor properties: `val cursor: StateFlow<Long?>`, `suspend fun setCursor(seq: Long)`, `fun clearCursor()`
  - Add settings properties: `val directDownloadsEnabled: StateFlow<Boolean>`, `suspend fun setDirectDownloadsEnabled(enabled: Boolean)`
  - Add permissions properties: `val permissions: StateFlow<Set<Permission>>`, `suspend fun setPermissions(permissions: Set<Permission>)`, `suspend fun addPermission(permission: Permission)`, `suspend fun removePermission(permission: Permission)`
  - Add likes placeholders: `val likedAlbums: StateFlow<Set<String>>`, `val likedArtists: StateFlow<Set<String>>`, `val likedTracks: StateFlow<Set<String>>`
  - Add full state load method: `suspend fun loadFullState(state: SyncState)`
  - Add clear method: `fun clear()`

### 1.5 Create SyncClient Interface
- [x] **Create `SyncClient.kt` in domain/sync**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncClient.kt`
  - Define `SyncClient` interface
  - Add property: `val connectionState: StateFlow<SyncConnectionState>`
  - Add methods: `suspend fun connect()`, `fun disconnect()`, `suspend fun sync(): Boolean`
  - Create `SyncConnectionState` enum with values: `Disconnected`, `Connecting`, `Connected`, `Reconnecting`
  - Document sync() behavior: fetches full state if cursor is null, otherwise fetches events since cursor

---

## Phase 2: Remote API Layer

### 2.1 Create Sync API Response Models
- [x] **Create `SyncResponses.kt` in remoteapi/internal/response**
  - File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/response/SyncResponses.kt`
  - Create `SyncStateResponse` with `@Serializable`: `seq: Long`, `likes: LikesJson`, `settings: List<SettingJson>`, `playlists: List<PlaylistJson>`, `permissions: List<String>`
  - Create `LikesJson` with `@Serializable`: `albums: List<String>`, `artists: List<String>`, `tracks: List<String>`
  - Create `SettingJson` with `@Serializable`: `key: String`, `value: JsonElement`
  - Create `PlaylistJson` with `@Serializable`: `id: String`, `name: String`, `tracks: List<String>`
  - Create `SyncEventsResponse` with `@Serializable`: `events: List<SyncEventJson>`, `@SerialName("current_seq") currentSeq: Long`
  - Create `SyncEventJson` with `@Serializable`: `seq: Long`, `type: String`, `payload: JsonElement`, `timestamp: Long`
  - Mark all classes as `internal`

### 2.2 Create Settings Request Model
- [x] **Create `UpdateUserSettingsRequest.kt` in remoteapi/internal/requests**
  - File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/requests/UpdateUserSettingsRequest.kt`
  - Create `UpdateUserSettingsRequest` with `@Serializable`: `settings: List<SettingJson>`
  - Mark class as `internal`
  - Reference `SettingJson` from SyncResponses.kt

### 2.3 Add Endpoints to RetrofitApiClient
- [x] **Update `RetrofitApiClient.kt` with sync endpoints**
  - File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RetrofitApiClient.kt`
  - Add `@GET("/v1/sync/state")` method `getSyncState(@Header("Authorization") authToken: String): Response<SyncStateResponse>`
  - Add `@GET("/v1/sync/events")` method `getSyncEvents(@Header("Authorization") authToken: String, @Query("since") since: Long): Response<SyncEventsResponse>`
  - Add `@PUT("/v1/user/settings")` method `updateUserSettings(@Header("Authorization") authToken: String, @Body request: UpdateUserSettingsRequest): Response<Unit>`
  - Import new request/response types

### 2.4 Add Methods to RemoteApiClient Interface
- [x] **Update `RemoteApiClient.kt` in domain/remoteapi**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/RemoteApiClient.kt`
  - Add method: `suspend fun getSyncState(): RemoteApiResponse<SyncState>`
  - Add method: `suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResult>`
  - Add method: `suspend fun updateUserSettings(settings: List<UserSettingDto>): RemoteApiResponse<Unit>`
  - Create `SyncEventsResult` data class: `events: List<SyncEvent>`, `currentSeq: Long`
  - Create `SyncEventsError` sealed interface with `data object EventsPruned : SyncEventsError` (for 410 Gone response)

### 2.5 Implement in RemoteApiClientImpl
- [x] **Update `RemoteApiClientImpl.kt` with sync implementations**
  - File: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt`
  - Implement `getSyncState()`: call Retrofit endpoint, map `SyncStateResponse` → `SyncState`
  - Parse permissions list using `Permission.fromSnakeCase()`
  - Map `LikesJson` → `LikedContent`, `SettingJson` → `UserSettingDto`, `PlaylistJson` → `PlaylistDto`
  - Implement `getSyncEvents()`: call Retrofit endpoint, map response to `SyncEventsResult`
  - Handle 410 response code → return `RemoteApiResponse.Error` with `SyncEventsError.EventsPruned`
  - Parse each `SyncEventJson` based on `type` field to create appropriate `SyncEvent` subtype
  - Implement `updateUserSettings()`: map `UserSettingDto` list to `UpdateUserSettingsRequest`, call endpoint

---

## Phase 3: Local Data Layer

### 3.1 Implement SyncStateStoreImpl
- [ ] **Create `SyncStateStoreImpl.kt` in localdata/internal/sync**
  - File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncStateStoreImpl.kt`
  - Create internal package: `localdata/internal/sync/`
  - Create class implementing `SyncStateStore`
  - Constructor: `context: Context`, `dispatcher: CoroutineDispatcher = Dispatchers.IO`
  - Initialize SharedPreferences with name "sync_state"
  - Implement cursor: `MutableStateFlow` backed by SharedPreferences Long (KEY_CURSOR), handle -1 as null
  - Implement `setCursor()`: update MutableStateFlow, persist with `commit()`
  - Implement `clearCursor()`: set flow to null, remove from SharedPreferences
  - Implement `directDownloadsEnabled`: `MutableStateFlow<Boolean>` backed by SharedPreferences (KEY_DIRECT_DOWNLOADS, default false)
  - Implement `setDirectDownloadsEnabled()`: update flow, persist with `commit()`
  - Implement permissions: `MutableStateFlow<Set<Permission>>` (in-memory only, refreshed on sync)
  - Implement `setPermissions()`, `addPermission()`, `removePermission()` updating the flow
  - Implement likes flows: `MutableStateFlow<Set<String>>` for albums, artists, tracks (placeholders)
  - Implement `loadFullState()`: dispatch to IO, call setCursor, iterate settings and set values, set permissions, set likes
  - Implement `clear()`: reset all flows to defaults, clear SharedPreferences
  - Add companion object with keys: KEY_CURSOR = "cursor", KEY_DIRECT_DOWNLOADS = "direct_downloads_enabled"

### 3.2 Create Event Parsing Helpers
- [ ] **Add event payload data classes in SyncClientImpl**
  - Create private `@Serializable` data classes for parsing event payloads:
  - `SettingChangedPayload(key: String, value: JsonElement)`
  - `PermissionPayload(permission: String)`
  - `PermissionsResetPayload(permissions: List<String>)`
  - `ContentLikePayload(contentType: String, contentId: String)` (for future use)

### 3.3 Implement SyncClientImpl
- [ ] **Create `SyncClientImpl.kt` in localdata/internal/sync**
  - File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncClientImpl.kt`
  - Create internal class implementing `SyncClient`
  - Constructor dependencies: `remoteApiClient: RemoteApiClient`, `syncStateStore: SyncStateStore`, `webSocketManager: WebSocketManager`, `authStore: AuthStore`, `dispatcher: CoroutineDispatcher`
  - Implement `connectionState` with `MutableStateFlow<SyncConnectionState>` starting at Disconnected
  - Private `syncJob: Job?` for tracking running sync
  - Implement `connect()`: check if already connected, set Connecting, call sync(), if fail set Disconnected and return, get auth state, connect WebSocket with url "/v1/sync/stream", set Connected
  - Implement `disconnect()`: cancel syncJob, disconnect WebSocket, set Disconnected
  - Implement `sync()`: get cursor from store, if null call `fullSync()`, else call `catchUp(cursor)`
  - Implement `fullSync()`: call `remoteApiClient.getSyncState()`, on success call `syncStateStore.loadFullState()`, return success boolean
  - Implement `catchUp(since: Long)`: call `remoteApiClient.getSyncEvents(since)`, on success iterate events and call `applyEvent()`, update cursor, return true
  - Handle 410 (EventsPruned): clear cursor, call `fullSync()`
  - Implement `applyEvent(event: SyncEvent)`: when SettingChanged and key="enable_direct_downloads" → call setDirectDownloadsEnabled; when PermissionGranted → parse and add; when PermissionRevoked → parse and remove; when PermissionsReset → parse and set all; update cursor after each event
  - Implement `handleSyncMessage(message: String)`: parse JSON to SyncEventEnvelope, call parseEvent, call applyEvent (in coroutine scope)
  - Implement `handleDisconnect()`: set Reconnecting, delay 1s, call sync()
  - Implement `parseEvent(envelope: SyncEventEnvelope): SyncEvent`: switch on type, decode payload, create appropriate SyncEvent subtype, return Unknown for unrecognized types

### 3.4 Update LocalDataModule
- [ ] **Update `LocalDataModule.kt` with sync providers**
  - File: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`
  - Add `@Provides @Singleton` function `provideSyncStateStore(@ApplicationContext context: Context): SyncStateStore` returning `SyncStateStoreImpl(context)`
  - Add `@Provides @Singleton` function `provideSyncClient(remoteApiClient, syncStateStore, webSocketManager, authStore): SyncClient` returning `SyncClientImpl(...)`
  - Add imports for new types
  - Ensure `WebSocketManager` is available (may need to create or verify it exists)

### 3.5 Create/Verify WebSocketManager
- [ ] **Verify or create WebSocketManager infrastructure**
  - Check if `WebSocketManager` already exists in localdata module
  - If not, create `WebSocketManager` interface in domain with: `fun connect(url, authToken, onMessage, onDisconnect)`, `fun disconnect()`
  - Create `WebSocketManagerImpl` in localdata using OkHttp WebSocket
  - Add to LocalDataModule as singleton provider
  - Handle reconnection logic, message parsing, connection state

---

## Phase 4: Use Cases

### 4.1 Update PerformLogin Use Case
- [ ] **Modify `PerformLogin.kt` to initialize sync after login**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogin.kt`
  - Add `SyncClient` as constructor dependency via `@Inject`
  - After successful login and storing auth state, call `syncClient.connect()`
  - This triggers full state fetch including settings and permissions
  - Note: Permissions are now in `SyncStateStore`, NOT in `AuthState.LoggedIn`

### 4.2 Create UpdateDirectDownloadsSetting Use Case
- [x] **Create `UpdateDirectDownloadsSetting.kt` in domain/settings/usecase**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/usecase/UpdateDirectDownloadsSetting.kt`
  - Create package: `domain/settings/usecase/`
  - Create class with `@Inject constructor(remoteApiClient: RemoteApiClient, syncStateStore: SyncStateStore)`
  - Create sealed interface `Result` with `data object Success` and `data object Error`
  - Implement `suspend operator fun invoke(enabled: Boolean): Result`
  - Save previous value: `val previousValue = syncStateStore.directDownloadsEnabled.value`
  - Optimistically update: `syncStateStore.setDirectDownloadsEnabled(enabled)`
  - Call remote: `remoteApiClient.updateUserSettings(listOf(UserSettingDto("enable_direct_downloads", enabled)))`
  - On success return `Result.Success`
  - On failure: revert `syncStateStore.setDirectDownloadsEnabled(previousValue)`, return `Result.Error`

### 4.3 Update PerformLogout Use Case
- [ ] **Modify `PerformLogout.kt` to clear sync state**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogout.kt`
  - Add `SyncClient` and `SyncStateStore` as constructor dependencies
  - Before clearing auth state: call `syncClient.disconnect()`
  - Then call `syncStateStore.clear()`
  - Finally call existing `authStore.clearAuthState()`

### 4.4 Create InitializeSync Use Case
- [ ] **Create `InitializeSync.kt` in domain/sync/usecase**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/usecase/InitializeSync.kt`
  - Create package: `domain/sync/usecase/`
  - Create class with `@Inject constructor(authStore: AuthStore, syncClient: SyncClient)`
  - Implement `suspend operator fun invoke()`
  - Get current auth state from `authStore.getAuthState().value`
  - If `authState is AuthState.LoggedIn`, call `syncClient.connect()`
  - Otherwise do nothing (user not logged in)

### 4.5 Update InitializeApp Use Case
- [ ] **Modify `InitializeApp.kt` to call InitializeSync**
  - File: `domain/src/main/java/com/lelloman/pezzottify/android/domain/usecase/InitializeApp.kt`
  - Add `InitializeSync` as constructor dependency
  - After auth state is restored/checked, call `initializeSync()`
  - This ensures sync is connected on app cold start if user is logged in

---

## Phase 5: UI Layer

### 5.1 Update SettingsScreenState
- [x] **Add new fields to `SettingsScreenState.kt`**
  - File: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenState.kt`
  - Add `directDownloadsEnabled: Boolean = false`
  - Add `hasDirectDownloadPermission: Boolean = false`
  - Add `isUpdatingDirectDownloads: Boolean = false`
  - Optionally add `syncConnected: Boolean = false` for sync status indicator

### 5.2 Update SettingsScreenActions
- [x] **Add action method to `SettingsScreenActions.kt`**
  - File: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenActions.kt`
  - Add method: `fun setDirectDownloadsEnabled(enabled: Boolean)`

### 5.3 Update SettingsScreenViewModel Interactor Interface
- [x] **Extend Interactor interface in `SettingsScreenViewModel.kt`**
  - File: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenViewModel.kt`
  - Add to `Interactor` interface: `fun getDirectDownloadsEnabled(): Boolean`
  - Add: `fun observeDirectDownloadsEnabled(): Flow<Boolean>`
  - Add: `suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean`
  - Add: `fun hasDirectDownloadPermission(): Boolean`
  - Add: `fun observePermissions(): Flow<Set<Permission>>`

### 5.4 Update SettingsScreenViewModel Implementation
- [x] **Implement new functionality in SettingsScreenViewModel**
  - In ViewModel init block, initialize state with: `directDownloadsEnabled = interactor.getDirectDownloadsEnabled()`, `hasDirectDownloadPermission = interactor.hasDirectDownloadPermission()`
  - Add launch block to observe `interactor.observeDirectDownloadsEnabled()` and update state on collect
  - Add launch block to observe `interactor.observePermissions()` and update `hasDirectDownloadPermission` based on `Permission.IssueContentDownload in permissions`
  - Implement action `setDirectDownloadsEnabled(enabled: Boolean)`: launch in viewModelScope, set `isUpdatingDirectDownloads = true`, call `interactor.setDirectDownloadsEnabled(enabled)`, set `isUpdatingDirectDownloads = false`

### 5.5 Update SettingsScreenInteractor
- [x] **Implement Interactor methods in `InteractorsModule.kt`**
  - File: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenInteractor.kt`
  - Add constructor dependencies: `syncStateStore: SyncStateStore`, `updateDirectDownloadsSetting: UpdateDirectDownloadsSetting`
  - Implement `getDirectDownloadsEnabled()`: return `syncStateStore.directDownloadsEnabled.value`
  - Implement `observeDirectDownloadsEnabled()`: return `syncStateStore.directDownloadsEnabled`
  - Implement `setDirectDownloadsEnabled(enabled)`: call `updateDirectDownloadsSetting(enabled)`, return true on Success, false on Error
  - Implement `hasDirectDownloadPermission()`: return `Permission.IssueContentDownload in syncStateStore.permissions.value`
  - Implement `observePermissions()`: return `syncStateStore.permissions`

### 5.6 Update SettingsScreen UI
- [x] **Add Direct Downloads toggle to `SettingsScreen.kt`**
  - File: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreen.kt`
  - Wrap new section in `if (state.hasDirectDownloadPermission) { ... }` to show only for users with permission
  - Add section header: `Text("Content Downloads", style = MaterialTheme.typography.titleLarge)` with `Spacer(24.dp)` before
  - Add Row with `Arrangement.SpaceBetween`, `Alignment.CenterVertically`
  - Left side: Column with title "Enable Direct Downloads" and subtitle "Automatically fetch missing content when browsing"
  - Right side: `Switch` with `checked = state.directDownloadsEnabled`, `onCheckedChange = { actions.setDirectDownloadsEnabled(it) }`, `enabled = !state.isUpdatingDirectDownloads`
  - Use appropriate Material 3 typography styles

---

## Phase 6: Testing

### 6.1 Unit Tests for Permission Parsing
- [ ] **Create `PermissionTest.kt` in domain tests**
  - File: `domain/src/test/java/com/lelloman/pezzottify/android/domain/auth/PermissionTest.kt`
  - Test `fromSnakeCase("access_catalog")` returns `Permission.AccessCatalog`
  - Test `fromSnakeCase("issue_content_download")` returns `Permission.IssueContentDownload`
  - Test `fromSnakeCase("unknown_permission")` returns null
  - Test all valid permission values are parsed correctly

### 6.2 Unit Tests for SyncStateStoreImpl
- [ ] **Create `SyncStateStoreImplTest.kt` in localdata tests**
  - File: `localdata/src/test/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncStateStoreImplTest.kt`
  - Test cursor persistence across instances
  - Test directDownloadsEnabled persistence
  - Test permissions update correctly (add, remove, reset)
  - Test `loadFullState()` populates all fields
  - Test `clear()` resets all state

### 6.3 Unit Tests for SyncClientImpl
- [ ] **Create `SyncClientImplTest.kt` in localdata tests**
  - File: `localdata/src/test/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncClientImplTest.kt`
  - Mock RemoteApiClient, SyncStateStore, WebSocketManager, AuthStore
  - Test `connect()` calls getSyncState on fresh login
  - Test `sync()` with null cursor does full sync
  - Test `sync()` with existing cursor does catch-up
  - Test 410 response triggers full sync
  - Test `applyEvent()` updates store correctly for each event type
  - Test event parsing for all known event types
  - Test Unknown event type handling

### 6.4 Unit Tests for UpdateDirectDownloadsSetting
- [x] **Create `UpdateDirectDownloadsSettingTest.kt` in domain tests**
  - File: `domain/src/test/java/com/lelloman/pezzottify/android/domain/settings/usecase/UpdateDirectDownloadsSettingTest.kt`
  - Test optimistic update is applied immediately
  - Test success returns Result.Success
  - Test failure reverts optimistic update
  - Test failure returns Result.Error

### 6.5 Integration Tests for Sync Endpoints
- [ ] **Add sync endpoint tests to remoteapi integration tests**
  - File: `remoteapi/src/integrationTest/java/.../SyncApiIntegrationTest.kt`
  - Test `GET /v1/sync/state` returns expected structure
  - Test `GET /v1/sync/events?since=0` returns events
  - Test `PUT /v1/user/settings` updates settings
  - Test 410 response for pruned cursor
  - Requires server-side SYNC implementation to be complete

---

## Phase 7: Final Integration & Cleanup

### 7.1 Verify Dependency Graph
- [ ] **Ensure all Hilt modules are properly connected**
  - Verify LocalDataModule provides all new dependencies
  - Check that RemoteApiClientImpl has new methods
  - Ensure SyncClient and SyncStateStore are injectable
  - Verify use cases can be injected into ViewModels/Interactors

### 7.2 Verify Login/Logout Flow
- [ ] **Test complete auth flow with sync**
  - Login triggers full sync, settings and permissions loaded
  - Settings UI reflects synced values
  - Logout disconnects sync and clears state
  - Re-login properly re-initializes sync

### 7.3 Verify App Cold Start
- [ ] **Test app startup when already logged in**
  - App launch calls InitializeApp → InitializeSync
  - Sync reconnects and catches up
  - Settings reflect latest server state

### 7.4 Remove Unused Code
- [ ] **Clean up any deprecated approaches**
  - If any old settings storage exists, remove it
  - Ensure no duplicate permission handling (permissions now in SyncStateStore only)
  - Remove any placeholder implementations

### 7.5 Documentation Update
- [ ] **Update CLAUDE.md and relevant docs**
  - Add sync infrastructure to architecture documentation
  - Document new use cases and their purposes
  - Note that permissions are now synced, not stored in AuthState

---

## Dependencies & Prerequisites

**Requires server-side SYNC implementation (see `/SYNC_PLAN.md`):**
- [ ] `GET /v1/sync/state` endpoint
- [ ] `GET /v1/sync/events` endpoint with 410 for pruned cursors
- [ ] `WS /v1/sync/stream` WebSocket endpoint
- [ ] `PUT /v1/user/settings` generating `setting_changed` event
- [ ] Admin endpoints generating permission events

**Module dependencies:**
- Phase 1 (domain) must complete before Phase 2, 3, 4
- Phase 2 (remoteapi) must complete before Phase 3
- Phase 3 (localdata) must complete before Phase 4
- Phase 4 (use cases) must complete before Phase 5
- Phase 5 (ui) can start after Phase 4 basics are done
- Phase 6 (testing) can run in parallel with later phases

---

## Notes

- The `enable_direct_downloads` setting is only visible to users with `IssueContentDownload` permission
- Permissions are no longer stored in `AuthState.LoggedIn` - they come from sync
- WebSocket connection provides real-time updates across devices
- Cursor-based sync with 410 fallback ensures no events are missed
- All settings changes use optimistic updates with rollback on failure
