# Android User Settings Implementation Plan

## Overview

Add server-synced user settings to the Android app, starting with the `enable_direct_downloads` setting. Settings are fetched on login and stored locally. The direct downloads setting is only visible to users with `IssueContentDownload` permission.

## API Contract

### Endpoints
- `GET /v1/user/settings` - Fetch all user settings
- `PUT /v1/user/settings` - Update user settings

### Request/Response Format
```json
// GET Response & PUT Request body
{
  "settings": [
    {"key": "enable_direct_downloads", "value": true}
  ]
}
```

### Login Response (already includes permissions)
```json
{
  "token": "...",
  "user_handle": "...",
  "permissions": ["AccessCatalog", "IssueContentDownload", ...]
}
```

---

## Implementation Steps

### Phase 1: Domain Layer - Models & Interfaces

#### 1.1 Create Permission enum
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/Permission.kt`

```kotlin
enum class Permission {
    AccessCatalog,
    LikeContent,
    OwnPlaylists,
    EditCatalog,
    ManagePermissions,
    IssueContentDownload,
    RebootServer,
    ViewAnalytics;

    companion object {
        fun fromString(value: String): Permission? = entries.find { it.name == value }
    }
}
```

#### 1.2 Update AuthState to include permissions
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/AuthState.kt`

```kotlin
@Serializable
data class LoggedIn(
    val userHandle: String,
    val authToken: String,
    val remoteUrl: String,
    val permissions: Set<Permission> = emptySet(),  // ADD THIS
) : AuthState
```

#### 1.3 Create RemoteUserSettingsStore interface
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/RemoteUserSettingsStore.kt`

```kotlin
interface RemoteUserSettingsStore {
    val directDownloadsEnabled: StateFlow<Boolean>

    suspend fun setDirectDownloadsEnabled(enabled: Boolean)
    suspend fun loadSettings(settings: List<UserSettingDto>)
    fun clear()
}
```

#### 1.4 Create UserSettingDto for API communication
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/UserSettingDto.kt`

```kotlin
data class UserSettingDto(
    val key: String,
    val value: Any,  // Boolean, String, etc.
)
```

---

### Phase 2: Remote API Layer

#### 2.1 Update LoginSuccessResponse
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/response/LoginSuccessResponse.kt`

```kotlin
@Serializable
data class LoginSuccessResponse(
    val token: String,
    @SerialName("user_handle") val userHandle: String,
    val permissions: List<String>,  // ADD: Will be parsed to Permission enum
)
```

#### 2.2 Create settings response models
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/response/UserSettingsResponse.kt`

```kotlin
@Serializable
internal data class UserSettingsResponse(
    val settings: List<UserSettingJson>,
)

@Serializable
internal data class UserSettingJson(
    val key: String,
    val value: JsonElement,  // Handle different value types
)
```

#### 2.3 Create settings request model
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/requests/UpdateUserSettingsRequest.kt`

```kotlin
@Serializable
internal data class UpdateUserSettingsRequest(
    val settings: List<UserSettingJson>,
)
```

#### 2.4 Add endpoints to RetrofitApiClient
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RetrofitApiClient.kt`

```kotlin
@GET("/v1/user/settings")
suspend fun getUserSettings(
    @Header("Authorization") authToken: String,
): Response<UserSettingsResponse>

@PUT("/v1/user/settings")
suspend fun updateUserSettings(
    @Header("Authorization") authToken: String,
    @Body request: UpdateUserSettingsRequest,
): Response<Unit>
```

#### 2.5 Add methods to RemoteApiClient interface
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/RemoteApiClient.kt`

```kotlin
suspend fun getUserSettings(): RemoteApiResponse<List<UserSettingDto>>
suspend fun updateUserSettings(settings: List<UserSettingDto>): RemoteApiResponse<Unit>
```

#### 2.6 Implement in RemoteApiClientImpl
**File**: `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt`

Implement the conversion from `UserSettingJson` to `UserSettingDto`, handling the `enable_direct_downloads` boolean value.

---

### Phase 3: Local Data Layer

#### 3.1 Implement RemoteUserSettingsStoreImpl
**File**: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/settings/RemoteUserSettingsStoreImpl.kt`

```kotlin
internal class RemoteUserSettingsStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : RemoteUserSettingsStore {

    private val prefs = context.getSharedPreferences(
        "remote_user_settings",
        Context.MODE_PRIVATE
    )

    private val mutableDirectDownloadsEnabled = MutableStateFlow(
        prefs.getBoolean(KEY_DIRECT_DOWNLOADS, false)
    )
    override val directDownloadsEnabled = mutableDirectDownloadsEnabled.asStateFlow()

    override suspend fun setDirectDownloadsEnabled(enabled: Boolean) {
        withContext(dispatcher) {
            mutableDirectDownloadsEnabled.value = enabled
            prefs.edit().putBoolean(KEY_DIRECT_DOWNLOADS, enabled).commit()
        }
    }

    override suspend fun loadSettings(settings: List<UserSettingDto>) {
        settings.forEach { setting ->
            when (setting.key) {
                "enable_direct_downloads" -> {
                    val enabled = setting.value as? Boolean ?: false
                    setDirectDownloadsEnabled(enabled)
                }
            }
        }
    }

    override fun clear() {
        mutableDirectDownloadsEnabled.value = false
        prefs.edit().clear().apply()
    }

    companion object {
        private const val KEY_DIRECT_DOWNLOADS = "direct_downloads_enabled"
    }
}
```

#### 3.2 Update LocalDataModule
**File**: `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/LocalDataModule.kt`

```kotlin
@Provides
@Singleton
fun provideRemoteUserSettingsStore(
    @ApplicationContext context: Context
): RemoteUserSettingsStore = RemoteUserSettingsStoreImpl(context)
```

---

### Phase 4: Use Cases

#### 4.1 Update PerformLogin use case
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogin.kt`

After successful login:
1. Parse permissions from response
2. Store permissions in AuthState
3. Fetch user settings via `remoteApiClient.getUserSettings()`
4. Load settings into `RemoteUserSettingsStore`

```kotlin
// After successful login
val permissions = remoteResponse.value.permissions
    .mapNotNull { Permission.fromString(it) }
    .toSet()

authStore.storeAuthState(
    AuthState.LoggedIn(
        userHandle = ...,
        authToken = ...,
        remoteUrl = ...,
        permissions = permissions,
    )
)

// Fetch and store user settings
when (val settingsResponse = remoteApiClient.getUserSettings()) {
    is RemoteApiResponse.Success -> {
        remoteUserSettingsStore.loadSettings(settingsResponse.value)
    }
    else -> { /* Log warning, continue */ }
}
```

#### 4.2 Create UpdateRemoteUserSetting use case
**File**: `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/usecase/UpdateDirectDownloadsSetting.kt`

```kotlin
class UpdateDirectDownloadsSetting @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val remoteUserSettingsStore: RemoteUserSettingsStore,
) : UseCase() {

    suspend operator fun invoke(enabled: Boolean): Result {
        // Optimistically update local store
        remoteUserSettingsStore.setDirectDownloadsEnabled(enabled)

        // Sync with server
        val response = remoteApiClient.updateUserSettings(
            listOf(UserSettingDto("enable_direct_downloads", enabled))
        )

        return when (response) {
            is RemoteApiResponse.Success -> Result.Success
            else -> {
                // Revert on failure
                remoteUserSettingsStore.setDirectDownloadsEnabled(!enabled)
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
Clear remote settings on logout:
```kotlin
remoteUserSettingsStore.clear()
```

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
}
```

In ViewModel init:
```kotlin
val initialState = SettingsScreenState(
    // ... existing ...
    directDownloadsEnabled = interactor.getDirectDownloadsEnabled(),
    hasDirectDownloadPermission = interactor.hasDirectDownloadPermission(),
)

// Observe changes
launch {
    interactor.observeDirectDownloadsEnabled().collect { enabled ->
        mutableState.update { it.copy(directDownloadsEnabled = enabled) }
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
    private val remoteUserSettingsStore: RemoteUserSettingsStore,
    private val updateDirectDownloadsSetting: UpdateDirectDownloadsSetting,
    private val authStore: AuthStore,
) : SettingsScreenViewModel.Interactor {

    override fun getDirectDownloadsEnabled(): Boolean =
        remoteUserSettingsStore.directDownloadsEnabled.value

    override fun observeDirectDownloadsEnabled(): Flow<Boolean> =
        remoteUserSettingsStore.directDownloadsEnabled

    override suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean {
        return when (updateDirectDownloadsSetting(enabled)) {
            UpdateDirectDownloadsSetting.Result.Success -> true
            UpdateDirectDownloadsSetting.Result.Error -> false
        }
    }

    override fun hasDirectDownloadPermission(): Boolean {
        val authState = authStore.getAuthState().value
        return authState is AuthState.LoggedIn &&
               authState.permissions.contains(Permission.IssueContentDownload)
    }
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
| **domain** | `auth/AuthState.kt` | UPDATE |
| **domain** | `settings/RemoteUserSettingsStore.kt` | CREATE |
| **domain** | `settings/UserSettingDto.kt` | CREATE |
| **domain** | `settings/usecase/UpdateDirectDownloadsSetting.kt` | CREATE |
| **domain** | `auth/usecase/PerformLogin.kt` | UPDATE |
| **domain** | `auth/usecase/PerformLogout.kt` | UPDATE |
| **domain** | `remoteapi/RemoteApiClient.kt` | UPDATE |
| **domain** | `remoteapi/response/LoginSuccessResponse.kt` | UPDATE |
| **remoteapi** | `internal/RetrofitApiClient.kt` | UPDATE |
| **remoteapi** | `internal/RemoteApiClientImpl.kt` | UPDATE |
| **remoteapi** | `internal/response/UserSettingsResponse.kt` | CREATE |
| **remoteapi** | `internal/requests/UpdateUserSettingsRequest.kt` | CREATE |
| **localdata** | `internal/settings/RemoteUserSettingsStoreImpl.kt` | CREATE |
| **localdata** | `LocalDataModule.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenState.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenActions.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenViewModel.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreenInteractor.kt` | UPDATE |
| **ui** | `screen/main/settings/SettingsScreen.kt` | UPDATE |

---

## Testing Considerations

1. **Unit tests** for:
   - `Permission.fromString()` parsing
   - `RemoteUserSettingsStoreImpl` persistence
   - `UpdateDirectDownloadsSetting` use case (success/failure paths)

2. **Integration tests** for:
   - Settings API endpoints (add to existing integration test suite)
   - Login flow with permissions parsing

3. **UI tests** for:
   - Settings section visibility based on permission
   - Toggle state changes

---

## Future Extensibility

The architecture supports adding new settings by:
1. Adding new field to `RemoteUserSettingsStore` interface
2. Handling new key in `loadSettings()` method
3. Creating use case for the new setting
4. Adding UI controls to settings screen

Settings keys are string-based, matching the backend `UserSetting` enum variants.
