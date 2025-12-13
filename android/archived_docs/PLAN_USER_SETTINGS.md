# Android User Settings Implementation Plan (Sync-Aware)

## Overview

Add server-synced user settings to the Android app, built on top of the multi-device sync infrastructure. This plan assumes the server-side SYNC feature (see `/SYNC_PLAN.md`) is already implemented.

**Key integration points with SYNC:**
- Initial state fetched via `GET /v1/sync/state` (not a dedicated settings endpoint)
- Real-time updates received via WebSocket sync stream
- Settings changes via `PUT /v1/user/settings` generate `setting_changed` sync events
- Cursor-based catch-up on reconnection

---

## API Contract (from SYNC)

### Full State Endpoint
`GET /v1/sync/state`

```json
{
  "seq": 42,
  "likes": {
    "albums": ["album_id_1"],
    "artists": ["artist_id_1"],
    "tracks": ["track_id_1"]
  },
  "settings": [
    {"key": "enable_direct_downloads", "value": true}
  ],
  "playlists": [...],
  "permissions": ["access_catalog", "like_content", "issue_content_download"]
}
```

### Events Since Endpoint
`GET /v1/sync/events?since={seq}`

```json
{
  "events": [
    {
      "seq": 43,
      "type": "setting_changed",
      "payload": {"key": "enable_direct_downloads", "value": false},
      "timestamp": 1701700005
    }
  ],
  "current_seq": 43
}
```

**Error:** Returns `410 Gone` if requested sequence is pruned → client must do full sync.

### Update Settings Endpoint
`PUT /v1/user/settings`

```json
{
  "settings": [
    {"key": "enable_direct_downloads", "value": true}
  ]
}
```

This generates a `setting_changed` event that broadcasts to other connected devices.

### WebSocket Sync Stream
`WS /v1/sync/stream`

Messages are JSON with same structure as events:
```json
{"seq": 44, "type": "setting_changed", "payload": {"key": "enable_direct_downloads", "value": true}, "timestamp": 1701700010}
```

---

## Implementation Steps

### Phase 1: Domain Layer - Sync Models & Interfaces

#### 1.1 Create Permission enum
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/Permission.kt`

```kotlin
enum class Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    ServerAdmin,
    ViewAnalytics;

    companion object {
        /**
         * Parse from snake_case server format (e.g., "access_catalog" -> AccessCatalog)
         */
        fun fromSnakeCase(value: String): Permission? {
            val pascalCase = value.split("_")
                .joinToString("") { it.replaceFirstChar { c -> c.uppercase() } }
            return entries.find { it.name == pascalCase }
        }
    }
}
```

#### 1.2 Create Sync Event types
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`

```kotlin
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class SyncEventEnvelope(
    val seq: Long,
    val type: String,
    val payload: JsonElement,
    val timestamp: Long,
)

sealed interface SyncEvent {
    val seq: Long

    data class SettingChanged(
        override val seq: Long,
        val key: String,
        val value: JsonElement,
    ) : SyncEvent

    data class ContentLiked(
        override val seq: Long,
        val contentType: String,
        val contentId: String,
    ) : SyncEvent

    data class ContentUnliked(
        override val seq: Long,
        val contentType: String,
        val contentId: String,
    ) : SyncEvent

    data class PermissionGranted(
        override val seq: Long,
        val permission: String,
    ) : SyncEvent

    data class PermissionRevoked(
        override val seq: Long,
        val permission: String,
    ) : SyncEvent

    data class PermissionsReset(
        override val seq: Long,
        val permissions: List<String>,
    ) : SyncEvent

    // Playlist events (for future use)
    data class PlaylistCreated(override val seq: Long, val playlistId: String, val name: String) : SyncEvent
    data class PlaylistRenamed(override val seq: Long, val playlistId: String, val name: String) : SyncEvent
    data class PlaylistDeleted(override val seq: Long, val playlistId: String) : SyncEvent
    data class PlaylistTrackAdded(override val seq: Long, val playlistId: String, val trackId: String, val position: Int) : SyncEvent
    data class PlaylistTrackRemoved(override val seq: Long, val playlistId: String, val trackId: String) : SyncEvent
    data class PlaylistTracksReordered(override val seq: Long, val playlistId: String, val trackIds: List<String>) : SyncEvent

    data class Unknown(override val seq: Long, val type: String) : SyncEvent
}
```

#### 1.3 Create Sync State model
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncState.kt`

```kotlin
data class SyncState(
    val seq: Long,
    val likes: LikedContent,
    val settings: List<UserSettingDto>,
    val playlists: List<PlaylistDto>,
    val permissions: Set<Permission>,
)

data class LikedContent(
    val albums: List<String>,
    val artists: List<String>,
    val tracks: List<String>,
)

data class PlaylistDto(
    val id: String,
    val name: String,
    val tracks: List<String>,
)

data class UserSettingDto(
    val key: String,
    val value: Any,  // Boolean, String, Number, etc.
)
```

#### 1.4 Create SyncStateStore interface
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncStateStore.kt`

```kotlin
interface SyncStateStore {
    // Cursor
    val cursor: StateFlow<Long?>
    suspend fun setCursor(seq: Long)
    fun clearCursor()

    // Settings
    val directDownloadsEnabled: StateFlow<Boolean>
    suspend fun setDirectDownloadsEnabled(enabled: Boolean)

    // Permissions (observed from sync, not stored in AuthState)
    val permissions: StateFlow<Set<Permission>>
    suspend fun setPermissions(permissions: Set<Permission>)
    suspend fun addPermission(permission: Permission)
    suspend fun removePermission(permission: Permission)

    // Likes (for future use, placeholder for now)
    val likedAlbums: StateFlow<Set<String>>
    val likedArtists: StateFlow<Set<String>>
    val likedTracks: StateFlow<Set<String>>

    // Full state load (from GET /v1/sync/state)
    suspend fun loadFullState(state: SyncState)

    // Clear all (on logout)
    fun clear()
}
```

#### 1.5 Create SyncClient interface
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncClient.kt`

```kotlin
interface SyncClient {
    val connectionState: StateFlow<SyncConnectionState>

    suspend fun connect()
    fun disconnect()

    /**
     * Perform initial sync or catch-up.
     * If cursor is null, fetches full state.
     * If cursor exists, fetches events since cursor.
     * Returns true if sync succeeded.
     */
    suspend fun sync(): Boolean
}

enum class SyncConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
```

---

### Phase 2: Remote API Layer

#### 2.1 Create Sync API response models
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/response/SyncResponses.kt`

```kotlin
@Serializable
internal data class SyncStateResponse(
    val seq: Long,
    val likes: LikesJson,
    val settings: List<SettingJson>,
    val playlists: List<PlaylistJson>,
    val permissions: List<String>,
)

@Serializable
internal data class LikesJson(
    val albums: List<String>,
    val artists: List<String>,
    val tracks: List<String>,
)

@Serializable
internal data class SettingJson(
    val key: String,
    val value: JsonElement,
)

@Serializable
internal data class PlaylistJson(
    val id: String,
    val name: String,
    val tracks: List<String>,
)

@Serializable
internal data class SyncEventsResponse(
    val events: List<SyncEventJson>,
    @SerialName("current_seq") val currentSeq: Long,
)

@Serializable
internal data class SyncEventJson(
    val seq: Long,
    val type: String,
    val payload: JsonElement,
    val timestamp: Long,
)
```

#### 2.2 Create settings request model
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/requests/UpdateUserSettingsRequest.kt`

```kotlin
@Serializable
internal data class UpdateUserSettingsRequest(
    val settings: List<SettingJson>,
)
```

#### 2.3 Add endpoints to RetrofitApiClient
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RetrofitApiClient.kt`

```kotlin
// Sync endpoints
@GET("/v1/sync/state")
suspend fun getSyncState(
    @Header("Authorization") authToken: String,
): Response<SyncStateResponse>

@GET("/v1/sync/events")
suspend fun getSyncEvents(
    @Header("Authorization") authToken: String,
    @Query("since") since: Long,
): Response<SyncEventsResponse>

// Settings update (generates sync event on server)
@PUT("/v1/user/settings")
suspend fun updateUserSettings(
    @Header("Authorization") authToken: String,
    @Body request: UpdateUserSettingsRequest,
): Response<Unit>
```

#### 2.4 Add methods to RemoteApiClient interface
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/RemoteApiClient.kt`

```kotlin
// Sync
suspend fun getSyncState(): RemoteApiResponse<SyncState>
suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResult>

// Settings
suspend fun updateUserSettings(settings: List<UserSettingDto>): RemoteApiResponse<Unit>

// Result type for events
data class SyncEventsResult(
    val events: List<SyncEvent>,
    val currentSeq: Long,
)

sealed interface SyncEventsError {
    data object EventsPruned : SyncEventsError  // 410 Gone
}
```

#### 2.5 Implement in RemoteApiClientImpl
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt`

Implement conversion from JSON responses to domain models:
- Parse `SyncStateResponse` → `SyncState`
- Parse `SyncEventsResponse` → `SyncEventsResult`
- Handle 410 response for `getSyncEvents` → return `EventsPruned` error
- Parse event types using `SyncEventJson.type` to create appropriate `SyncEvent` subtypes

---

### Phase 3: Local Data Layer

#### 3.1 Implement SyncStateStoreImpl
**File**: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncStateStoreImpl.kt`

```kotlin
internal class SyncStateStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : SyncStateStore {

    private val prefs = context.getSharedPreferences("sync_state", Context.MODE_PRIVATE)

    // Cursor
    private val mutableCursor = MutableStateFlow(
        prefs.getLong(KEY_CURSOR, -1L).takeIf { it >= 0 }
    )
    override val cursor = mutableCursor.asStateFlow()

    override suspend fun setCursor(seq: Long) = withContext(dispatcher) {
        mutableCursor.value = seq
        prefs.edit().putLong(KEY_CURSOR, seq).commit()
    }

    override fun clearCursor() {
        mutableCursor.value = null
        prefs.edit().remove(KEY_CURSOR).apply()
    }

    // Settings
    private val mutableDirectDownloadsEnabled = MutableStateFlow(
        prefs.getBoolean(KEY_DIRECT_DOWNLOADS, false)
    )
    override val directDownloadsEnabled = mutableDirectDownloadsEnabled.asStateFlow()

    override suspend fun setDirectDownloadsEnabled(enabled: Boolean) = withContext(dispatcher) {
        mutableDirectDownloadsEnabled.value = enabled
        prefs.edit().putBoolean(KEY_DIRECT_DOWNLOADS, enabled).commit()
    }

    // Permissions
    private val mutablePermissions = MutableStateFlow<Set<Permission>>(emptySet())
    override val permissions = mutablePermissions.asStateFlow()

    override suspend fun setPermissions(permissions: Set<Permission>) {
        mutablePermissions.value = permissions
        // Permissions don't need persistence - refreshed on each sync
    }

    override suspend fun addPermission(permission: Permission) {
        mutablePermissions.value = mutablePermissions.value + permission
    }

    override suspend fun removePermission(permission: Permission) {
        mutablePermissions.value = mutablePermissions.value - permission
    }

    // Likes (placeholder - will be expanded in future)
    private val mutableLikedAlbums = MutableStateFlow<Set<String>>(emptySet())
    private val mutableLikedArtists = MutableStateFlow<Set<String>>(emptySet())
    private val mutableLikedTracks = MutableStateFlow<Set<String>>(emptySet())
    override val likedAlbums = mutableLikedAlbums.asStateFlow()
    override val likedArtists = mutableLikedArtists.asStateFlow()
    override val likedTracks = mutableLikedTracks.asStateFlow()

    // Full state load
    override suspend fun loadFullState(state: SyncState) = withContext(dispatcher) {
        // Cursor
        setCursor(state.seq)

        // Settings
        state.settings.forEach { setting ->
            when (setting.key) {
                "enable_direct_downloads" -> {
                    val enabled = setting.value as? Boolean ?: false
                    setDirectDownloadsEnabled(enabled)
                }
            }
        }

        // Permissions
        setPermissions(state.permissions)

        // Likes
        mutableLikedAlbums.value = state.likes.albums.toSet()
        mutableLikedArtists.value = state.likes.artists.toSet()
        mutableLikedTracks.value = state.likes.tracks.toSet()
    }

    override fun clear() {
        mutableCursor.value = null
        mutableDirectDownloadsEnabled.value = false
        mutablePermissions.value = emptySet()
        mutableLikedAlbums.value = emptySet()
        mutableLikedArtists.value = emptySet()
        mutableLikedTracks.value = emptySet()
        prefs.edit().clear().apply()
    }

    companion object {
        private const val KEY_CURSOR = "cursor"
        private const val KEY_DIRECT_DOWNLOADS = "direct_downloads_enabled"
    }
}
```

#### 3.2 Implement SyncClientImpl
**File**: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/sync/SyncClientImpl.kt`

```kotlin
internal class SyncClientImpl(
    private val remoteApiClient: RemoteApiClient,
    private val syncStateStore: SyncStateStore,
    private val webSocketManager: WebSocketManager,  // Reuse existing WebSocket infrastructure
    private val authStore: AuthStore,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : SyncClient {

    private val mutableConnectionState = MutableStateFlow(SyncConnectionState.Disconnected)
    override val connectionState = mutableConnectionState.asStateFlow()

    private var syncJob: Job? = null

    override suspend fun connect() = withContext(dispatcher) {
        if (mutableConnectionState.value == SyncConnectionState.Connected) return@withContext

        mutableConnectionState.value = SyncConnectionState.Connecting

        // Initial sync
        val syncSuccess = sync()
        if (!syncSuccess) {
            mutableConnectionState.value = SyncConnectionState.Disconnected
            return@withContext
        }

        // Connect WebSocket for real-time updates
        val authState = authStore.getAuthState().value as? AuthState.LoggedIn
            ?: return@withContext

        webSocketManager.connect(
            url = "${authState.remoteUrl.replace("http", "ws")}/v1/sync/stream",
            authToken = authState.authToken,
            onMessage = { message -> handleSyncMessage(message) },
            onDisconnect = { handleDisconnect() },
        )

        mutableConnectionState.value = SyncConnectionState.Connected
    }

    override fun disconnect() {
        syncJob?.cancel()
        webSocketManager.disconnect()
        mutableConnectionState.value = SyncConnectionState.Disconnected
    }

    override suspend fun sync(): Boolean = withContext(dispatcher) {
        val cursor = syncStateStore.cursor.value

        if (cursor == null) {
            // No cursor - do full sync
            fullSync()
        } else {
            // Have cursor - do catch-up
            catchUp(cursor)
        }
    }

    private suspend fun fullSync(): Boolean {
        return when (val response = remoteApiClient.getSyncState()) {
            is RemoteApiResponse.Success -> {
                syncStateStore.loadFullState(response.value)
                true
            }
            else -> false
        }
    }

    private suspend fun catchUp(since: Long): Boolean {
        return when (val response = remoteApiClient.getSyncEvents(since)) {
            is RemoteApiResponse.Success -> {
                response.value.events.forEach { event ->
                    applyEvent(event)
                }
                syncStateStore.setCursor(response.value.currentSeq)
                true
            }
            is RemoteApiResponse.Error -> {
                if (response.error is SyncEventsError.EventsPruned) {
                    // Events pruned - need full sync
                    syncStateStore.clearCursor()
                    fullSync()
                } else {
                    false
                }
            }
            else -> false
        }
    }

    private suspend fun applyEvent(event: SyncEvent) {
        when (event) {
            is SyncEvent.SettingChanged -> {
                when (event.key) {
                    "enable_direct_downloads" -> {
                        val enabled = (event.value as? JsonPrimitive)?.booleanOrNull ?: false
                        syncStateStore.setDirectDownloadsEnabled(enabled)
                    }
                }
            }
            is SyncEvent.PermissionGranted -> {
                Permission.fromSnakeCase(event.permission)?.let {
                    syncStateStore.addPermission(it)
                }
            }
            is SyncEvent.PermissionRevoked -> {
                Permission.fromSnakeCase(event.permission)?.let {
                    syncStateStore.removePermission(it)
                }
            }
            is SyncEvent.PermissionsReset -> {
                val permissions = event.permissions.mapNotNull { Permission.fromSnakeCase(it) }.toSet()
                syncStateStore.setPermissions(permissions)
            }
            is SyncEvent.ContentLiked -> { /* TODO: implement likes sync */ }
            is SyncEvent.ContentUnliked -> { /* TODO: implement likes sync */ }
            else -> { /* Ignore unknown/playlist events for now */ }
        }

        // Update cursor
        syncStateStore.setCursor(event.seq)
    }

    private fun handleSyncMessage(message: String) {
        // Parse and apply event
        CoroutineScope(dispatcher).launch {
            try {
                val envelope = Json.decodeFromString<SyncEventEnvelope>(message)
                val event = parseEvent(envelope)
                applyEvent(event)
            } catch (e: Exception) {
                // Log error, continue
            }
        }
    }

    private fun handleDisconnect() {
        mutableConnectionState.value = SyncConnectionState.Reconnecting
        // WebSocketManager handles reconnection, we just need to catch up when reconnected
        CoroutineScope(dispatcher).launch {
            delay(1000)  // Brief delay before catch-up
            sync()
        }
    }

    private fun parseEvent(envelope: SyncEventEnvelope): SyncEvent {
        return when (envelope.type) {
            "setting_changed" -> {
                val payload = Json.decodeFromJsonElement<SettingChangedPayload>(envelope.payload)
                SyncEvent.SettingChanged(envelope.seq, payload.key, payload.value)
            }
            "permission_granted" -> {
                val payload = Json.decodeFromJsonElement<PermissionPayload>(envelope.payload)
                SyncEvent.PermissionGranted(envelope.seq, payload.permission)
            }
            "permission_revoked" -> {
                val payload = Json.decodeFromJsonElement<PermissionPayload>(envelope.payload)
                SyncEvent.PermissionRevoked(envelope.seq, payload.permission)
            }
            "permissions_reset" -> {
                val payload = Json.decodeFromJsonElement<PermissionsResetPayload>(envelope.payload)
                SyncEvent.PermissionsReset(envelope.seq, payload.permissions)
            }
            // Add other event types as needed
            else -> SyncEvent.Unknown(envelope.seq, envelope.type)
        }
    }
}

// Payload data classes for parsing
@Serializable
private data class SettingChangedPayload(val key: String, val value: JsonElement)
@Serializable
private data class PermissionPayload(val permission: String)
@Serializable
private data class PermissionsResetPayload(val permissions: List<String>)
```

#### 3.3 Update LocalDataModule
**File**: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`

```kotlin
@Provides
@Singleton
fun provideSyncStateStore(
    @ApplicationContext context: Context
): SyncStateStore = SyncStateStoreImpl(context)

@Provides
@Singleton
fun provideSyncClient(
    remoteApiClient: RemoteApiClient,
    syncStateStore: SyncStateStore,
    webSocketManager: WebSocketManager,
    authStore: AuthStore,
): SyncClient = SyncClientImpl(
    remoteApiClient = remoteApiClient,
    syncStateStore = syncStateStore,
    webSocketManager = webSocketManager,
    authStore = authStore,
)
```

---

### Phase 4: Use Cases

#### 4.1 Update PerformLogin use case
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogin.kt`

After successful login, initialize sync:

```kotlin
class PerformLogin @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val authStore: AuthStore,
    private val syncClient: SyncClient,
) {
    suspend operator fun invoke(/*...*/): Result {
        // ... existing login logic ...

        // After successful login, store auth state
        authStore.storeAuthState(
            AuthState.LoggedIn(
                userHandle = response.userHandle,
                authToken = response.token,
                remoteUrl = remoteUrl,
            )
        )

        // Initialize sync (fetches full state including settings & permissions)
        syncClient.connect()

        return Result.Success
    }
}
```

**Note:** Permissions are now managed by `SyncStateStore`, not stored in `AuthState`. The `AuthState.LoggedIn` does NOT need a `permissions` field.

#### 4.2 Create UpdateDirectDownloadsSetting use case
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/usecase/UpdateDirectDownloadsSetting.kt`

```kotlin
class UpdateDirectDownloadsSetting @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val syncStateStore: SyncStateStore,
) {
    suspend operator fun invoke(enabled: Boolean): Result {
        val previousValue = syncStateStore.directDownloadsEnabled.value

        // Optimistically update local store
        syncStateStore.setDirectDownloadsEnabled(enabled)

        // Sync with server (this generates a sync event for other devices)
        val response = remoteApiClient.updateUserSettings(
            listOf(UserSettingDto("enable_direct_downloads", enabled))
        )

        return when (response) {
            is RemoteApiResponse.Success -> Result.Success
            else -> {
                // Revert on failure
                syncStateStore.setDirectDownloadsEnabled(previousValue)
                Result.Error
            }
        }
    }

    sealed interface Result {
        data object Success : Result
        data object Error : Result
    }
}
```

#### 4.3 Update PerformLogout use case
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogout.kt`

```kotlin
class PerformLogout @Inject constructor(
    private val authStore: AuthStore,
    private val syncClient: SyncClient,
    private val syncStateStore: SyncStateStore,
) {
    suspend operator fun invoke() {
        // Disconnect sync
        syncClient.disconnect()

        // Clear sync state
        syncStateStore.clear()

        // Clear auth state
        authStore.clearAuthState()
    }
}
```

#### 4.4 Create InitializeSync use case (for app startup)
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/usecase/InitializeSync.kt`

```kotlin
class InitializeSync @Inject constructor(
    private val authStore: AuthStore,
    private val syncClient: SyncClient,
) {
    suspend operator fun invoke() {
        val authState = authStore.getAuthState().value
        if (authState is AuthState.LoggedIn) {
            syncClient.connect()
        }
    }
}
```

Call this from `InitializeApp` use case after auth state is restored.

---

### Phase 5: UI Layer

#### 5.1 Update SettingsScreenState
**File**: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenState.kt`

```kotlin
data class SettingsScreenState(
    // ... existing fields ...
    val directDownloadsEnabled: Boolean = false,
    val hasDirectDownloadPermission: Boolean = false,
    val isUpdatingDirectDownloads: Boolean = false,
    val syncConnected: Boolean = false,  // Optional: show sync status
)
```

#### 5.2 Update SettingsScreenActions
**File**: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenActions.kt`

```kotlin
interface SettingsScreenActions {
    // ... existing methods ...
    fun setDirectDownloadsEnabled(enabled: Boolean)
}
```

#### 5.3 Update SettingsScreenViewModel
**File**: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenViewModel.kt`

Update `Interactor` interface:
```kotlin
interface Interactor {
    // ... existing methods ...
    fun getDirectDownloadsEnabled(): Boolean
    fun observeDirectDownloadsEnabled(): Flow<Boolean>
    suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean
    fun hasDirectDownloadPermission(): Boolean
    fun observePermissions(): Flow<Set<Permission>>
}
```

In ViewModel init:
```kotlin
val initialState = SettingsScreenState(
    // ... existing ...
    directDownloadsEnabled = interactor.getDirectDownloadsEnabled(),
    hasDirectDownloadPermission = interactor.hasDirectDownloadPermission(),
)

// Observe settings changes (from sync)
launch {
    interactor.observeDirectDownloadsEnabled().collect { enabled ->
        mutableState.update { it.copy(directDownloadsEnabled = enabled) }
    }
}

// Observe permission changes (from sync - admin may grant/revoke)
launch {
    interactor.observePermissions().collect { permissions ->
        mutableState.update {
            it.copy(hasDirectDownloadPermission = Permission.IssueContentDownload in permissions)
        }
    }
}
```

Add action handler:
```kotlin
override fun setDirectDownloadsEnabled(enabled: Boolean) {
    viewModelScope.launch {
        mutableState.update { it.copy(isUpdatingDirectDownloads = true) }
        interactor.setDirectDownloadsEnabled(enabled)
        mutableState.update { it.copy(isUpdatingDirectDownloads = false) }
    }
}
```

#### 5.4 Update SettingsScreenInteractor
**File**: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreenInteractor.kt`

```kotlin
class SettingsScreenInteractor @Inject constructor(
    // ... existing dependencies ...
    private val syncStateStore: SyncStateStore,
    private val updateDirectDownloadsSetting: UpdateDirectDownloadsSetting,
) : SettingsScreenViewModel.Interactor {

    override fun getDirectDownloadsEnabled(): Boolean =
        syncStateStore.directDownloadsEnabled.value

    override fun observeDirectDownloadsEnabled(): Flow<Boolean> =
        syncStateStore.directDownloadsEnabled

    override suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean {
        return when (updateDirectDownloadsSetting(enabled)) {
            UpdateDirectDownloadsSetting.Result.Success -> true
            UpdateDirectDownloadsSetting.Result.Error -> false
        }
    }

    override fun observePermissions(): Flow<Set<Permission>> =
        syncStateStore.permissions
}
```

#### 5.5 Update SettingsScreen UI
**File**: `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/main/settings/SettingsScreen.kt`

Add new section after existing sections (only if user has permission):

```kotlin
// Content Downloads Section (conditional)
if (state.hasDirectDownloadPermission) {
    item {
        Spacer(modifier = Modifier.height(24.dp))
        Text("Content Downloads", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(16.dp))
    }

    item {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    "Enable Direct Downloads",
                    style = MaterialTheme.typography.bodyLarge
                )
                Text(
                    "Automatically fetch missing content when browsing",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Switch(
                checked = state.directDownloadsEnabled,
                onCheckedChange = { actions.setDirectDownloadsEnabled(it) },
                enabled = !state.isUpdatingDirectDownloads
            )
        }
    }
}
```

---

## File Summary

| Layer | File | Action |
|-------|------|--------|
| **domain** | `auth/Permission.kt` | CREATE |
| **domain** | `sync/SyncEvent.kt` | CREATE |
| **domain** | `sync/SyncState.kt` | CREATE |
| **domain** | `sync/SyncStateStore.kt` | CREATE |
| **domain** | `sync/SyncClient.kt` | CREATE |
| **domain** | `sync/usecase/InitializeSync.kt` | CREATE |
| **domain** | `settings/usecase/UpdateDirectDownloadsSetting.kt` | CREATE |
| **domain** | `auth/usecase/PerformLogin.kt` | UPDATE |
| **domain** | `auth/usecase/PerformLogout.kt` | UPDATE |
| **domain** | `usecase/InitializeApp.kt` | UPDATE |
| **domain** | `remoteapi/RemoteApiClient.kt` | UPDATE |
| **remoteapi** | `internal/RetrofitApiClient.kt` | UPDATE |
| **remoteapi** | `internal/RemoteApiClientImpl.kt` | UPDATE |
| **remoteapi** | `internal/response/SyncResponses.kt` | CREATE |
| **remoteapi** | `internal/requests/UpdateUserSettingsRequest.kt` | CREATE |
| **localdata** | `internal/sync/SyncStateStoreImpl.kt` | CREATE |
| **localdata** | `internal/sync/SyncClientImpl.kt` | CREATE |
| **localdata** | `LocalDataModule.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenState.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenActions.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenViewModel.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenInteractor.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreen.kt` | UPDATE |

---

## Sync Flow Summary

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              LOGIN FLOW                                  │
├─────────────────────────────────────────────────────────────────────────┤
│  1. User logs in → POST /v1/auth/login                                  │
│  2. Store auth token                                                     │
│  3. SyncClient.connect()                                                │
│     └─→ GET /v1/sync/state → load full state (settings, permissions)   │
│     └─→ Connect WebSocket /v1/sync/stream                               │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                          SETTINGS CHANGE FLOW                            │
├─────────────────────────────────────────────────────────────────────────┤
│  Device A:                                                               │
│  1. User toggles setting                                                 │
│  2. Optimistic local update                                              │
│  3. PUT /v1/user/settings → server logs event                           │
│  4. Server broadcasts event to other devices                             │
│                                                                          │
│  Device B (connected via WebSocket):                                     │
│  1. Receives setting_changed event                                       │
│  2. Applies to local state                                               │
│  3. UI updates automatically via StateFlow                               │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                          RECONNECTION FLOW                               │
├─────────────────────────────────────────────────────────────────────────┤
│  1. WebSocket disconnects (network loss, app backgrounded)              │
│  2. On reconnect: GET /v1/sync/events?since={cursor}                    │
│     └─→ If 200: apply missed events, update cursor                      │
│     └─→ If 410: cursor too old, do full sync                            │
│  3. Resume WebSocket listening                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Key Differences from Previous Plan

| Aspect | Previous Plan | Sync-Aware Plan |
|--------|---------------|-----------------|
| Initial fetch | `GET /v1/user/settings` on login | `GET /v1/sync/state` (full state) |
| Permissions | Stored in `AuthState.LoggedIn` | Managed by `SyncStateStore`, updated via events |
| Real-time updates | Not supported | WebSocket for instant cross-device sync |
| Reconnection | Not addressed | Cursor-based catch-up with 410 fallback |
| State store | `RemoteUserSettingsStore` (settings only) | `SyncStateStore` (settings + permissions + likes) |

---

## Testing Considerations

1. **Unit tests** for:
   - `Permission.fromSnakeCase()` parsing
   - `SyncStateStoreImpl` state management and persistence
   - `SyncClientImpl` event parsing and application
   - `UpdateDirectDownloadsSetting` optimistic update + rollback

2. **Integration tests** for:
   - Sync endpoints (`/v1/sync/state`, `/v1/sync/events`)
   - Settings update generates sync event
   - 410 response triggers full sync

3. **End-to-end tests** for:
   - Fresh login → full sync → settings visible
   - Change setting on web → Android receives via WebSocket
   - App backgrounded → foregrounded → catch-up sync works
   - Long offline → 410 → full sync recovery

---

## Future Extensibility

The sync infrastructure supports adding new synced data by:
1. Adding new fields to `SyncStateStore`
2. Handling new event types in `SyncClientImpl.applyEvent()`
3. Exposing state via appropriate interfaces (e.g., likes store)

Already scaffolded for:
- Liked content (albums, artists, tracks)
- Playlist events (create, rename, delete, track add/remove/reorder)

---

## Dependencies

**Requires server-side SYNC implementation:**
- `GET /v1/sync/state` endpoint
- `GET /v1/sync/events` endpoint with 410 for pruned cursors
- `WS /v1/sync/stream` WebSocket endpoint
- `PUT /v1/user/settings` generating `setting_changed` event
- Admin endpoints generating permission events

See `/SYNC_PLAN.md` for server implementation details.
