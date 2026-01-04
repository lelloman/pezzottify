# Plan: External Album Screen Feature (Android)

## Status

**Completed:** Phases 1-2 (Server + Remote API Layer)
**Remaining:** Phases 3-5 (Android Domain/UI/Testing)

---

## Overview

Add the ability to view external (not-yet-downloaded) albums in a dedicated screen on Android, with download request functionality and live status updates via WebSocket.

## Key Design Decisions

1. **No External Artist Screen** - When navigating to an artist that's not in catalog, we insert it on-the-fly and show the regular `ArtistScreen`.
2. **WebSocket for Status Updates** - Reuse existing sync event infrastructure.
3. **Feature Gating** - Requires `RequestContent` permission AND `externalSearchEnabled` setting.
4. **Same IDs** - Internal catalog and external provider use the same IDs.

---

## Phase 3: Android Domain Layer

### 3.1 Add New Response Models

**File:** `android/domain/src/main/java/.../domain/remoteapi/response/`

```kotlin
data class ExternalAlbumDetails(
    val id: String,
    val name: String,
    val artistId: String,
    val artistName: String,
    val imageUrl: String?,
    val year: Int?,
    val albumType: String?,
    val totalTracks: Int,
    val tracks: List<ExternalTrack>,
    val inCatalog: Boolean,
    val requestStatus: RequestStatusInfo?,
)

data class ExternalTrack(
    val id: String,
    val name: String,
    val trackNumber: Int,
    val discNumber: Int?,
    val durationMs: Long?,
)

data class RequestStatusInfo(
    val requestId: String,
    val status: DownloadQueueStatus,
    val queuePosition: Int?,
    val progress: DownloadProgress?,
    val errorMessage: String?,
    val createdAt: Long,
)

data class ExternalArtistDiscography(
    val artist: ExternalArtist,
    val albums: List<DiscographyAlbum>,
)

data class ExternalArtist(
    val id: String,
    val name: String,
    val imageUrl: String?,
)

data class DiscographyAlbum(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val year: Int?,
    val albumType: String?,
    val totalTracks: Int?,
    val inCatalog: Boolean,
    val requestStatus: RequestStatusInfo?,
)
```

### 3.2 Add New Use Cases

**File:** `android/domain/src/main/java/.../domain/download/`

```kotlin
class GetExternalAlbumDetailsUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(albumId: String): Result<ExternalAlbumDetails>
}

class GetExternalArtistDiscographyUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(artistId: String): Result<ExternalArtistDiscography>
}
```

### 3.3 Add Download Status Observable

**File:** `android/domain/src/main/java/.../domain/download/`

```kotlin
interface DownloadStatusRepository {
    fun observeStatus(contentId: String): Flow<RequestStatusInfo?>
    fun observeAllUpdates(): Flow<DownloadStatusUpdate>
}

sealed class DownloadStatusUpdate {
    data class Created(val event: DownloadRequestCreated) : DownloadStatusUpdate()
    data class StatusChanged(val event: DownloadStatusChanged) : DownloadStatusUpdate()
    data class ProgressUpdated(val event: DownloadProgressUpdated) : DownloadStatusUpdate()
    data class Completed(val event: DownloadCompleted) : DownloadStatusUpdate()
}
```

### 3.4 Add Feature Flag Check

```kotlin
class IsExternalSearchEnabledUseCase @Inject constructor(
    private val sessionStore: SessionStore,
    private val userSettingsStore: UserSettingsStore,
) {
    suspend operator fun invoke(): Boolean {
        val hasPermission = sessionStore.getPermissions().contains(Permission.RequestContent)
        val settingEnabled = userSettingsStore.getSyncedSetting(UserSetting.ExternalSearchEnabled)
        return hasPermission && settingEnabled
    }

    fun observe(): Flow<Boolean>
}
```

---

## Phase 4: Android UI Layer

### 4.1 ExternalAlbumScreen

**Files:**
- `android/ui/src/main/java/.../ui/screen/main/externalalbum/ExternalAlbumScreen.kt`
- `android/ui/src/main/java/.../ui/screen/main/externalalbum/ExternalAlbumScreenViewModel.kt`
- `android/ui/src/main/java/.../ui/screen/main/externalalbum/ExternalAlbumScreenState.kt`

**UI Structure:**
```
┌─────────────────────────────────┐
│ ← Back                          │
├─────────────────────────────────┤
│ [Album Cover Image]             │
│ Album Name                      │
│ Artist Name (clickable)         │
│ 2023 • Album • 12 tracks        │
├─────────────────────────────────┤
│ [REQUEST DOWNLOAD]              │
│   or                            │
│ Status: Pending #3 in queue     │
│ [████████░░] 8/12 tracks        │
│   or                            │
│ [✓ IN YOUR CATALOG] → navigate  │
├─────────────────────────────────┤
│ TRACKS                          │
│ 1. Track One              3:45  │
│ 2. Track Two              4:12  │
│ ...                             │
└─────────────────────────────────┘
```

**State:**
```kotlin
data class ExternalAlbumScreenState(
    val isLoading: Boolean = true,
    val album: ExternalAlbumDetails? = null,
    val requestStatus: RequestStatusInfo? = null,
    val isRequesting: Boolean = false,
    val error: String? = null,
)
```

### 4.2 Update Search Results Screen

**Changes:**
1. Remove inline "Request" button from external search result items
2. Add checkmark icon overlay for `inCatalog` items
3. Make entire item clickable
4. Click navigation: `inCatalog` → `AlbumScreen`, else → `ExternalAlbumScreen`

### 4.3 Enhance In-Catalog ArtistScreen

**Changes:**
1. If external search enabled, fetch external discography
2. Filter out albums already in catalog
3. Display "Available to Download" section below catalog albums
4. Clicking external album → Navigate to `ExternalAlbumScreen`

**State additions:**
```kotlin
data class ArtistScreenState(
    // ... existing fields ...
    val externalSearchEnabled: Boolean = false,
    val externalAlbums: List<DiscographyAlbumUi> = emptyList(),
    val isLoadingExternal: Boolean = false,
    val externalError: String? = null,
)
```

### 4.4 Update MyRequestsScreen Navigation

**Changes:**
- `Completed` → Navigate to regular `AlbumScreen`
- Other statuses → Navigate to `ExternalAlbumScreen`
- Subscribe to `DownloadStatusRepository.observeAllUpdates()` for live updates

### 4.5 Add Navigation Routes

```kotlin
object ExternalAlbumRoute {
    const val route = "external_album/{albumId}"
    const val albumIdArg = "albumId"
    fun createRoute(albumId: String) = "external_album/$albumId"
}
```

---

## Phase 5: Testing & Polish

### 5.1 Unit Tests

- `GetExternalAlbumDetailsUseCaseTest`
- `GetExternalArtistDiscographyUseCaseTest`
- `IsExternalSearchEnabledUseCaseTest`
- `ExternalAlbumScreenViewModelTest`
- `DownloadStatusRepositoryTest`

### 5.2 Edge Cases

1. Album completes while viewing → Live update transitions to "In Your Catalog"
2. Permission revoked while viewing → Show error
3. Network errors → Show error state with retry
4. Rate limit reached → Show error message

### 5.3 UI Polish

1. Loading skeleton for external album screen
2. Smooth state transitions
3. Pull-to-refresh
4. Consistent styling with existing screens

---

## Implementation Order

1. **Phase 3** - Domain layer (models, use cases, repository)
2. **Phase 4.1** - ExternalAlbumScreen
3. **Phase 4.2** - Update search results navigation
4. **Phase 4.3** - Enhance ArtistScreen with external albums
5. **Phase 4.4** - Update MyRequestsScreen navigation
6. **Phase 4.5** - Add navigation routes
7. **Phase 5** - Testing & polish
