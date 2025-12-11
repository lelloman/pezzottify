# Plan: External Album Screen Feature

## Overview

Add the ability to view external (not-yet-downloaded) albums in a dedicated screen, with download request functionality and live status updates via WebSocket. This creates a seamless experience where users can explore content from the external provider, see album details with tracks, and request downloads with real-time status feedback.

## Key Design Decisions

1. **No External Artist Screen** - When navigating to an artist that's not in catalog, we insert it on-the-fly (like Direct Downloads) and show the regular `ArtistScreen`. This simplifies the implementation significantly.

2. **WebSocket for Status Updates** - Reuse existing sync event infrastructure to push download status changes to clients. No polling needed.

3. **Feature Gating** - External search features require: `user.hasPermission(RequestContent) AND userSettings.externalSearchEnabled`

4. **Same IDs** - Internal catalog and external provider use the same IDs, no mapping needed.

5. **Rate Limit Handling** - Don't show rate limit status proactively; just show error if request fails due to rate limit.

6. **Retry Handled Server-Side** - Users can't retry failed downloads; they'd need to request again (admins can retry via admin panel).

## Feature Requirements

### Core Requirements
1. Search results become clickable (remove inline "Request" button, show checkmark if in catalog)
2. Clicking external album → `ExternalAlbumScreen` with metadata, tracks, request button, and live status
3. Clicking artist name on external album → Insert artist to catalog if needed, navigate to regular `ArtistScreen`
4. External album screen shows request status with live updates via WebSocket
5. In-catalog `ArtistScreen` shows "Available to Download" section with external albums (when feature enabled)
6. My Requests items navigate to `ExternalAlbumScreen` (pending/in-progress/failed) or regular `AlbumScreen` (completed)

### Coexistence with Direct Downloads
- Direct Downloads (`enable_direct_downloads` + `IssueContentDownload` permission) remains as-is
- External Search (`enable_external_search` + `RequestContent` permission) is a separate feature
- Both can coexist; users may have one, both, or neither enabled

---

## Phase 1: Server Enhancements (catalog-server - Rust) [DONE]

### 1.1 Add External Album Details Endpoint [DONE]

**New endpoint:** `GET /v1/download/album/:album_id`

**Purpose:** Get detailed metadata for a single external album, including track listing.

**Implementation:**
- Call downloader client's album metadata endpoint
- Call downloader client's album tracks endpoint
- Combine into single response
- Enrich with `in_catalog` (check catalog store) and `request_status` (check queue store by content_id)

**Response structure:**
```rust
pub struct ExternalAlbumDetails {
    pub id: String,
    pub name: String,
    pub artist_id: String,
    pub artist_name: String,
    pub image_url: Option<String>,
    pub year: Option<i32>,
    pub album_type: Option<String>,  // "album", "single", "compilation"
    pub total_tracks: i32,
    pub tracks: Vec<ExternalTrack>,
    pub in_catalog: bool,
    pub request_status: Option<RequestStatusInfo>,
}

pub struct ExternalTrack {
    pub id: String,
    pub name: String,
    pub track_number: i32,
    pub disc_number: Option<i32>,
    pub duration_ms: Option<i64>,
}

pub struct RequestStatusInfo {
    pub request_id: String,
    pub status: DownloadQueueStatus,
    pub queue_position: Option<i32>,
    pub progress: Option<DownloadProgress>,
    pub error_message: Option<String>,
    pub created_at: i64,
}
```

**Files to modify:**
- `catalog-server/src/download_manager/models.rs` - Add new structs
- `catalog-server/src/download_manager/search_proxy.rs` - Add method to fetch album details
- `catalog-server/src/server/server.rs` - Add route handler

### 1.2 Enhance Discography Endpoint [DONE]

**Existing endpoint:** `GET /v1/download/search/discography/:artist_id`

**Enhancement:** Add request status for each album in the response.

**Updated response structure:**
```rust
pub struct DiscographyResult {
    pub artist: SearchResult,
    pub albums: Vec<DiscographyAlbum>,
}

pub struct DiscographyAlbum {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub year: Option<i32>,
    pub album_type: Option<String>,
    pub total_tracks: Option<i32>,
    pub in_catalog: bool,
    pub request_status: Option<RequestStatusInfo>,  // NEW
}
```

**Files to modify:**
- `catalog-server/src/download_manager/models.rs` - Update structs
- `catalog-server/src/download_manager/search_proxy.rs` - Enrich with request status
- `catalog-server/src/server/server.rs` - Update handler

### 1.3 Add WebSocket Messages for Download Status [DONE]

**New sync event types** (added to existing sync event system):

```rust
// New variants for UserEvent enum
pub enum UserEvent {
    // ... existing variants ...

    // Download status events
    DownloadRequestCreated {
        request_id: String,
        content_id: String,
        content_type: DownloadContentType,
        content_name: String,
        artist_name: Option<String>,
        queue_position: i32,
    },
    DownloadStatusChanged {
        request_id: String,
        content_id: String,
        status: DownloadQueueStatus,
        queue_position: Option<i32>,
        error_message: Option<String>,
    },
    DownloadProgressUpdated {
        request_id: String,
        content_id: String,
        progress: DownloadProgress,
    },
    DownloadCompleted {
        request_id: String,
        content_id: String,
        // content_id is also the catalog_id since IDs are the same
    },
}
```

**Implementation notes:**
- Reuse existing WebSocket sync event infrastructure
- Events are user-scoped (sent to the user who made the request)
- Download manager / queue processor emits events when status changes
- Events flow through existing `UserEventStore` → WebSocket broadcast path

**Files to modify:**
- `catalog-server/src/user/sync_events.rs` - Add new event variants
- `catalog-server/src/download_manager/queue_processor.rs` - Emit events on status changes
- No new WebSocket handler code needed - reuses existing sync infrastructure

---

## Phase 2: Android Remote API Layer [DONE]

### 2.1 Add New API Methods [DONE]

**File:** `android/remoteapi/src/main/java/.../RemoteApiClient.kt`

```kotlin
// New methods
suspend fun getExternalAlbumDetails(albumId: String): RemoteApiResponse<ExternalAlbumDetailsResponse>

suspend fun getExternalArtistDiscography(artistId: String): RemoteApiResponse<ExternalArtistDiscographyResponse>
```

### 2.2 Add Response DTOs [DONE]

**File:** `android/remoteapi/src/main/java/.../dto/`

```kotlin
data class ExternalAlbumDetailsDto(
    val id: String,
    val name: String,
    val artistId: String,
    val artistName: String,
    val imageUrl: String?,
    val year: Int?,
    val albumType: String?,
    val totalTracks: Int,
    val tracks: List<ExternalTrackDto>,
    val inCatalog: Boolean,
    val requestStatus: RequestStatusInfoDto?,
)

data class ExternalTrackDto(
    val id: String,
    val name: String,
    val trackNumber: Int,
    val discNumber: Int?,
    val durationMs: Long?,
)

data class RequestStatusInfoDto(
    val requestId: String,
    val status: String,  // Maps to DownloadQueueStatus
    val queuePosition: Int?,
    val progress: DownloadProgressDto?,
    val errorMessage: String?,
    val createdAt: Long,
)

data class ExternalArtistDiscographyDto(
    val artist: ExternalArtistDto,
    val albums: List<DiscographyAlbumDto>,
)

data class ExternalArtistDto(
    val id: String,
    val name: String,
    val imageUrl: String?,
)

data class DiscographyAlbumDto(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val year: Int?,
    val albumType: String?,
    val totalTracks: Int?,
    val inCatalog: Boolean,
    val requestStatus: RequestStatusInfoDto?,
)
```

### 2.3 Handle New Sync Event Types [DONE]

**Files to modify:**
- `android/domain/src/main/java/.../domain/sync/SyncEvent.kt` - Add new event types
- `android/domain/src/main/java/.../domain/sync/SyncManagerImpl.kt` - Handle new events

New sync event types to add:
```kotlin
sealed class SyncEvent {
    // ... existing events ...

    data class DownloadRequestCreated(
        val requestId: String,
        val contentId: String,
        val contentType: DownloadContentType,
        val contentName: String,
        val artistName: String?,
        val queuePosition: Int,
    ) : SyncEvent()

    data class DownloadStatusChanged(
        val requestId: String,
        val contentId: String,
        val status: DownloadQueueStatus,
        val queuePosition: Int?,
        val errorMessage: String?,
    ) : SyncEvent()

    data class DownloadProgressUpdated(
        val requestId: String,
        val contentId: String,
        val progress: DownloadProgress,
    ) : SyncEvent()

    data class DownloadCompleted(
        val requestId: String,
        val contentId: String,
    ) : SyncEvent()
}
```

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
/**
 * Repository that exposes download status updates from sync events.
 */
interface DownloadStatusRepository {
    /**
     * Observe status updates for a specific content ID.
     * Emits whenever a sync event updates this content's status.
     */
    fun observeStatus(contentId: String): Flow<RequestStatusInfo?>

    /**
     * Observe all download status updates (for My Requests screen).
     */
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

**File:** `android/domain/src/main/java/.../domain/download/`

```kotlin
class IsExternalSearchEnabledUseCase @Inject constructor(
    private val sessionStore: SessionStore,  // or wherever permissions are stored
    private val userSettingsStore: UserSettingsStore,
) {
    suspend operator fun invoke(): Boolean {
        val hasPermission = sessionStore.getPermissions().contains(Permission.RequestContent)
        val settingEnabled = userSettingsStore.getSyncedSetting(UserSetting.ExternalSearchEnabled)
        return hasPermission && settingEnabled
    }

    fun observe(): Flow<Boolean>  // For reactive UI updates
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
│                                 │
│ Album Name                      │
│ Artist Name (clickable)         │
│ 2023 • Album • 12 tracks        │
├─────────────────────────────────┤
│ [REQUEST DOWNLOAD]  ← Button    │
│   or                            │
│ Status: Pending #3 in queue     │
│ [████████░░] 8/12 tracks        │
│   or                            │
│ [✓ IN YOUR CATALOG] → navigate  │
├─────────────────────────────────┤
│ TRACKS                          │
│ 1. Track One              3:45  │
│ 2. Track Two              4:12  │
│ 3. Track Three            3:33  │
│ ...                             │
└─────────────────────────────────┘
```

**State:**
```kotlin
data class ExternalAlbumScreenState(
    val isLoading: Boolean = true,
    val album: ExternalAlbumDetails? = null,
    val requestStatus: RequestStatusInfo? = null,
    val isRequesting: Boolean = false,  // Loading state for request button
    val error: String? = null,
)

sealed class ExternalAlbumScreenEvent {
    object RequestDownload : ExternalAlbumScreenEvent()
    object NavigateToArtist : ExternalAlbumScreenEvent()
    object NavigateToCatalogAlbum : ExternalAlbumScreenEvent()
}
```

**ViewModel behavior:**
1. Load album details on init (REST call)
2. Subscribe to `DownloadStatusRepository.observeStatus(albumId)` for live updates
3. Handle request button click → call `RequestAlbumDownloadUseCase`
4. On error from request, show error message (including rate limit errors)
5. Artist name click → Navigate to regular `ArtistScreen` (artist will be created on-the-fly if needed via Direct Downloads or we can pre-insert)
6. When status becomes `Completed` or `inCatalog` becomes true → Show "In Your Catalog" button to navigate

### 4.2 Update Search Results Screen

**Files to modify:**
- `android/ui/src/main/java/.../ui/screen/main/search/` (search results display)

**Changes:**
1. Remove inline "Request" button from external search result items
2. Add checkmark icon overlay for `inCatalog` items
3. Add subtle "queued" indicator for `inQueue` items (optional, could be icon badge)
4. Make entire item clickable
5. Click navigation logic:
   - `inCatalog == true` → Navigate to regular `AlbumScreen`
   - `inCatalog == false` → Navigate to `ExternalAlbumScreen`

**Updated item UI:**
```
┌─────────────────────────────────┐
│ [img] Album Name           [✓]  │  ← Checkmark if in catalog
│       Artist Name               │     (or queue icon if in queue)
│       2023                      │
└─────────────────────────────────┘
```

### 4.3 Enhance In-Catalog ArtistScreen

**Files to modify:**
- `android/ui/src/main/java/.../ui/screen/main/artist/ArtistScreen.kt`
- `android/ui/src/main/java/.../ui/screen/main/artist/ArtistScreenViewModel.kt`
- `android/ui/src/main/java/.../ui/screen/main/artist/ArtistScreenState.kt`

**Changes:**
1. Inject `IsExternalSearchEnabledUseCase`
2. If enabled, fetch external discography using `GetExternalArtistDiscographyUseCase`
3. Filter out albums already in catalog (compare by ID)
4. Display "Available to Download" section below catalog albums
5. Each external album item shows: image, name, year, and request status (if any)
6. Clicking external album → Navigate to `ExternalAlbumScreen`

**Updated UI structure:**
```
┌─────────────────────────────────┐
│ [Artist Image]                  │
│ Artist Name                     │
├─────────────────────────────────┤
│ DISCOGRAPHY                     │  ← Existing section
│ [Album 1] [Album 2] [Album 3]   │
├─────────────────────────────────┤
│ AVAILABLE TO DOWNLOAD           │  ← NEW section (if feature enabled)
│ [Album 4 ↓] [Album 5 ↓]         │     ↓ indicates downloadable
│ [Album 6 ⏳]                     │     ⏳ indicates in queue
└─────────────────────────────────┘
```

**State additions:**
```kotlin
data class ArtistScreenState(
    // ... existing fields ...
    val externalSearchEnabled: Boolean = false,
    val externalAlbums: List<DiscographyAlbumUi> = emptyList(),
    val isLoadingExternal: Boolean = false,
    val externalError: String? = null,
)

data class DiscographyAlbumUi(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val year: Int?,
    val status: ExternalAlbumStatus,
)

sealed class ExternalAlbumStatus {
    object Available : ExternalAlbumStatus()  // Can request
    data class InQueue(val queuePosition: Int?) : ExternalAlbumStatus()
    data class InProgress(val progress: DownloadProgress) : ExternalAlbumStatus()
    object Failed : ExternalAlbumStatus()
}
```

### 4.4 Update MyRequestsScreen Navigation

**Files to modify:**
- `android/ui/src/main/java/.../ui/screen/main/myrequests/MyRequestsScreen.kt`
- `android/ui/src/main/java/.../ui/screen/main/myrequests/MyRequestsScreenViewModel.kt`

**Changes:**
1. Make request items clickable (they may already be partially clickable)
2. Navigation logic based on status:
   - `Completed` → Navigate to regular `AlbumScreen` (contentId == catalogId)
   - `Pending`, `InProgress`, `RetryWaiting`, `Failed` → Navigate to `ExternalAlbumScreen`
3. Subscribe to `DownloadStatusRepository.observeAllUpdates()` for live status updates (in addition to existing refresh mechanism)

### 4.5 Add Navigation Routes

**Files to modify:**
- `android/ui/src/main/java/.../ui/navigation/` (navigation graph)

**New route:**
```kotlin
// In navigation sealed class or route definitions
object ExternalAlbumRoute {
    const val route = "external_album/{albumId}"
    const val albumIdArg = "albumId"

    fun createRoute(albumId: String) = "external_album/$albumId"
}
```

---

## Phase 5: Testing & Polish

### 5.1 Unit Tests

**Server:**
- Test external album details endpoint
- Test discography endpoint with request status enrichment
- Test sync event emission on status changes

**Android:**
- `GetExternalAlbumDetailsUseCaseTest`
- `GetExternalArtistDiscographyUseCaseTest`
- `IsExternalSearchEnabledUseCaseTest`
- `ExternalAlbumScreenViewModelTest`
- `DownloadStatusRepositoryTest`

### 5.2 Integration Tests

- Server endpoint tests for new/modified endpoints
- Android integration tests for API calls

### 5.3 Edge Cases to Handle

1. **Album completes while viewing:** Live update via sync event transitions UI to "In Your Catalog" state
2. **Permission revoked while viewing:** Next API call fails; show error or navigate away
3. **Network errors:** Show error state with retry option (refresh)
4. **External API unavailable:** Show error message from server
5. **Rate limit reached:** Show error message from request response
6. **Already requested:** Show current status instead of request button (from initial load)

### 5.4 UI Polish

1. Loading skeleton for external album screen
2. Smooth state transitions (request button → pending status → progress → completed)
3. Pull-to-refresh for manual status refresh
4. Consistent styling with existing album/artist screens
5. Appropriate loading indicators when fetching external data on artist screen

---

## Implementation Order

1. **Server: Phase 1.1** - External album details endpoint
2. **Server: Phase 1.2** - Enhance discography endpoint with request status
3. **Server: Phase 1.3** - Add sync events for download status
4. **Android: Phase 2.1-2.2** - Remote API methods and DTOs
5. **Android: Phase 2.3** - Handle new sync event types
6. **Android: Phase 3** - Domain layer (models, use cases, repository)
7. **Android: Phase 4.1** - ExternalAlbumScreen
8. **Android: Phase 4.2** - Update search results navigation
9. **Android: Phase 4.3** - Enhance ArtistScreen with external albums
10. **Android: Phase 4.4** - Update MyRequestsScreen navigation
11. **Android: Phase 4.5** - Add navigation routes
12. **Phase 5** - Testing & polish

---

## Dependencies / Notes

1. **Downloader API:** Has two endpoints (album metadata + album tracks). Server will combine them.

2. **Sync Events:** Reuses existing WebSocket sync infrastructure. No new subscriptions needed.

3. **IDs:** Internal catalog and external provider use same IDs. No mapping needed.

4. **Rate Limiting:** Server returns error on rate limit; client shows error message. No proactive rate limit display.

5. **Retry:** Server-side only (admin panel). Users cannot retry; they'd need to re-request.

6. **Direct Downloads Feature:** Remains separate and unchanged. Both features can coexist.

---

## Estimated Scope

- **Server changes:** ~8-12 files modified/created
- **Android changes:** ~20-25 files modified/created
- **New screens:** 1 (ExternalAlbumScreen)
- **Modified screens:** 3 (SearchScreen, ArtistScreen, MyRequestsScreen)
