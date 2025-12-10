# Android Double-Search Feature Plan

## Overview

Implement external search (via downloader service) alongside the existing catalog search on Android, similar to the web implementation. This allows users with `RequestContent` permission to search external providers and request content downloads.

## Web Implementation Summary (for reference)

- **Settings**: Toggle `enable_external_search` in user settings (only visible with RequestContent permission)
- **Layout**: Side-by-side on desktop (>1024px), stacked on mobile
- **API calls**: Parallel catalog + external searches, plus limits check
- **External results**: Show relevance-sorted list with "In Catalog", "In Queue", or "Request" button states
- **My Requests**: Separate page to track pending/completed downloads

---

## Android Implementation Plan

### Phase 1: API Layer (remoteapi module)

Add new API endpoints to `RetrofitApiClient` and `RemoteApiClient`:

1. **External Search**
   ```kotlin
   GET /v1/download/search?q={query}&type={type}
   // type = "album" | "artist"
   // Returns: { results: [ExternalSearchResult] }
   ```

2. **Download Limits**
   ```kotlin
   GET /v1/download/limits
   // Returns: { requests_today, max_per_day, can_request, in_queue, max_queue }
   ```

3. **Request Album Download**
   ```kotlin
   POST /v1/download/request/album
   // Body: { album_id, album_name, artist_name }
   ```

4. **My Requests**
   ```kotlin
   GET /v1/download/my-requests
   // Returns list of user's queued/completed requests
   ```

**New data classes:**
- `ExternalSearchResult` (id, name, artist_name, year, image_url, in_catalog, in_queue, score)
- `DownloadLimits` (requests_today, max_per_day, can_request, in_queue, max_queue)
- `DownloadRequest` (for my-requests response)
- `RequestAlbumBody`

---

### Phase 2: Domain Layer (domain module)

Add new use cases:

1. **PerformExternalSearchUseCase**
   - Takes query and search type (album/artist)
   - Returns `Result<List<ExternalSearchResult>>`

2. **GetDownloadLimitsUseCase**
   - Returns `Result<DownloadLimits>`

3. **RequestAlbumDownloadUseCase**
   - Takes album_id, album_name, artist_name
   - Returns `Result<Unit>` (success/failure)

4. **GetMyDownloadRequestsUseCase**
   - Returns `Result<List<DownloadRequest>>`

5. **CheckCanRequestContentUseCase** (or extend existing permission checking)
   - Check if current user has RequestContent permission
   - Might already exist in auth/session handling

---

### Phase 3: Settings Integration

1. **Add external search toggle to Settings screen**
   - Only visible when user has RequestContent permission
   - Uses existing user settings sync mechanism
   - Key: `enable_external_search` (same as web for consistency)

2. **Extend UserSettingsStore** (or equivalent)
   - Add `isExternalSearchEnabled` property
   - Sync with backend user settings

---

### Phase 4: UI Layer - Search Screen Updates

**Decision: In-Search Toggle (Option F)**

Add a toggle/checkbox directly in the search screen that switches between catalog and external search modes. This is the simplest and most intuitive approach for mobile.

#### How it works:
- Toggle appears in/near the SearchBar (only visible when user has permission AND setting enabled)
- **OFF (default)**: Normal catalog search behavior
- **ON**: Search queries go to external API, results show with Request buttons
- Toggle state persists across searches (within session, or saved to preferences)
- Single results list - no tabs, no split views, no complexity

#### UI placement: Inside SearchBar trailing area
- Compact toggle/icon in the SearchBar's trailing section
- Only visible when: user has RequestContent permission AND external search setting enabled
- Clear visual distinction between catalog mode (default) and external mode

#### Benefits:
- Simple mental model: "I'm searching catalog" vs "I'm searching for new music"
- One list of results at a time - clean UI
- Explicit user control over which source to search
- No wasted screen space on empty sections
- Easy to understand and implement

#### Behavior:
- When toggled ON and user types, debounced query goes to external search API
- External results displayed with: image, name, artist, year, status/action
- Limits shown at top of results (e.g., "2/5 today · 1/3 in queue")
- When toggled OFF, reverts to normal catalog search
- "In Catalog" items are tappable → navigate to catalog album/artist screen

#### Filter Chips (new addition for both modes):
- Row of filter chips below SearchBar
- **Catalog mode**: Album, Artist, Track
- **External mode**: Album, Artist (no Track - API limitation)
- Chips are multi-select or single-select (TBD)
- When no filter selected: search all types

---

### Phase 5: External Search Result UI

1. **ExternalSearchResultContent** sealed class
   - `Album(id, name, artistName, year, imageUrl, inCatalog, inQueue)`
   - `Artist(id, name, imageUrl, inCatalog, inQueue)`

2. **ExternalSearchResultCard** composable
   - Thumbnail image (from URL, not catalog)
   - Name, artist name (for albums), year
   - Status badge OR Request button:
     - "In Catalog" (green badge) → navigates to catalog item
     - "In Queue" (orange badge)
     - "Request" (button) → calls RequestAlbumDownloadUseCase

3. **Limits display**
   - Show at top of external results section
   - Format: "X/Y today · X/Y in queue"
   - Visual warning when at limit

---

### Phase 6: My Requests Screen

New screen accessible from Profile/Settings:

1. **MyRequestsScreen** composable
   - Header with current limits
   - Pending section: List with queue position, progress indicator
   - Completed section: Grid/list of completed downloads
     - Clickable to navigate to catalog item
   - Failed section: Show error messages

2. **MyRequestsViewModel**
   - Load requests via GetMyDownloadRequestsUseCase
   - Refresh on pull-to-refresh
   - Handle loading/error states

---

### Phase 7: State Management Updates

Extend `SearchScreenViewModel`:

1. **New state fields:**
   ```kotlin
   data class SearchScreenState(
       // existing fields...
       val canUseExternalSearch: Boolean = false,  // permission + setting check
       val isExternalMode: Boolean = false,        // toggle state (persisted locally)
       val externalResults: List<ExternalSearchResult>? = null,
       val externalSearchLoading: Boolean = false,
       val externalSearchError: String? = null,
       val downloadLimits: DownloadLimits? = null,
       val selectedFilters: Set<SearchFilter> = emptySet(),  // filter chips
   )

   enum class SearchFilter { Album, Artist, Track }
   ```

2. **New actions:**
   ```kotlin
   fun toggleExternalMode()                        // flip between catalog/external
   fun toggleFilter(filter: SearchFilter)          // add/remove filter chip
   fun requestAlbumDownload(result: ExternalSearchResult)
   fun onExternalResultClick(result: ExternalSearchResult)  // navigate if in catalog
   fun refreshLimits()
   ```

3. **Search execution logic:**
   - **Catalog mode**: Call catalog search API with selected filters
   - **External mode**:
     - Call external search API for each selected type (album/artist)
     - Fetch limits in parallel
     - Merge and sort results by score
   - Both respect filter chip selection

---

## Task List

### Phase 1: API Layer (remoteapi module)

- [ ] **1.1** Create `ExternalSearchResult` data class in remoteapi
  - Fields: id, name, artist_name, year, image_url, in_catalog, in_queue, score, type (album/artist)
- [ ] **1.2** Create `ExternalSearchResponse` data class (wrapper with results list)
- [ ] **1.3** Create `DownloadLimits` data class
  - Fields: requests_today, max_per_day, can_request, in_queue, max_queue
- [ ] **1.4** Create `DownloadRequest` data class for my-requests response
  - Fields: id, album_id, album_name, artist_name, status, progress, error_message, created_at, catalog_album_id (nullable)
- [ ] **1.5** Create `RequestAlbumBody` data class
  - Fields: album_id, album_name, artist_name
- [ ] **1.6** Add `externalSearch(query, type)` to `RetrofitApiClient` interface
  - GET /v1/download/search with query params
- [ ] **1.7** Add `getDownloadLimits()` to `RetrofitApiClient` interface
  - GET /v1/download/limits
- [ ] **1.8** Add `requestAlbumDownload(body)` to `RetrofitApiClient` interface
  - POST /v1/download/request/album
- [ ] **1.9** Add `getMyRequests()` to `RetrofitApiClient` interface
  - GET /v1/download/my-requests
- [ ] **1.10** Implement all 4 methods in `RemoteApiClientImpl`
- [ ] **1.11** Add corresponding methods to `RemoteApiClient` interface

### Phase 2: Domain Layer (domain module)

- [ ] **2.1** Create `ExternalSearchResult` domain model (mirror of API model or map from it)
- [ ] **2.2** Create `DownloadLimits` domain model
- [ ] **2.3** Create `DownloadRequest` domain model with status enum (Pending, InProgress, Completed, Failed)
- [ ] **2.4** Create `PerformExternalSearchUseCase`
  - Input: query (String), type (Album/Artist)
  - Output: Result<List<ExternalSearchResult>>
- [ ] **2.5** Create `GetDownloadLimitsUseCase`
  - Output: Result<DownloadLimits>
- [ ] **2.6** Create `RequestAlbumDownloadUseCase`
  - Input: albumId, albumName, artistName
  - Output: Result<Unit>
- [ ] **2.7** Create `GetMyDownloadRequestsUseCase`
  - Output: Result<List<DownloadRequest>>

### Phase 3: Settings Integration

- [ ] **3.1** Add `enable_external_search` key constant to user settings
- [ ] **3.2** Add `isExternalSearchEnabled` property to UserSettingsStore (or equivalent)
  - Persist locally (not synced to server)
- [ ] **3.3** Add `setExternalSearchEnabled(enabled)` method
- [ ] **3.4** Check if `RequestContent` permission is exposed in session/auth state
  - If not, add `hasRequestContentPermission` to auth state
- [ ] **3.5** Add external search toggle row to Settings screen composable
  - Only visible when hasRequestContentPermission is true
- [ ] **3.6** Wire toggle to UserSettingsStore

### Phase 4: Filter Chips for Catalog Search

- [ ] **4.1** Create `SearchFilter` enum in ui module (Album, Artist, Track)
- [ ] **4.2** Add `selectedFilters: Set<SearchFilter>` to `SearchScreenState`
- [ ] **4.3** Add `toggleFilter(filter)` action to `SearchScreenActions`
- [ ] **4.4** Implement `toggleFilter` in `SearchScreenViewModel`
- [ ] **4.5** Create `SearchFilterChips` composable (row of FilterChip components)
- [ ] **4.6** Add `SearchFilterChips` to `SearchScreen` below SearchBar
- [ ] **4.7** Update search logic to pass filters to catalog search API
- [ ] **4.8** Test filter chips with catalog search

### Phase 5: External Mode Toggle

- [ ] **5.1** Add `canUseExternalSearch: Boolean` to `SearchScreenState`
  - Computed from: hasRequestContentPermission AND isExternalSearchEnabled
- [ ] **5.2** Add `isExternalMode: Boolean` to `SearchScreenState`
- [ ] **5.3** Add `externalModeEnabled` to local preferences (persist toggle state)
- [ ] **5.4** Add `toggleExternalMode()` action to `SearchScreenActions`
- [ ] **5.5** Implement `toggleExternalMode` in `SearchScreenViewModel`
  - Persist to local preferences
  - Clear current results when switching modes
- [ ] **5.6** Create external mode toggle icon/button composable
- [ ] **5.7** Add toggle to SearchBar trailing area (only visible when canUseExternalSearch)
- [ ] **5.8** Update filter chips visibility: hide Track chip when in external mode
- [ ] **5.9** Wire search logic to call external API when isExternalMode is true

### Phase 6: External Search Result UI

- [ ] **6.1** Create `ExternalSearchResultContent` sealed class in ui module
  - Album(id, name, artistName, year, imageUrl, inCatalog, inQueue, catalogId?)
  - Artist(id, name, imageUrl, inCatalog, inQueue, catalogId?)
- [ ] **6.2** Add external search state fields to `SearchScreenState`
  - externalResults: List<ExternalSearchResultContent>?
  - externalSearchLoading: Boolean
  - externalSearchError: String?
  - downloadLimits: DownloadLimits?
- [ ] **6.3** Create `ExternalAlbumSearchResult` composable
  - Image (from URL), name, artist, year
  - Status badge or Request button
- [ ] **6.4** Create `ExternalArtistSearchResult` composable
  - Image (from URL), name
  - Status badge or Request button
- [ ] **6.5** Create `DownloadLimitsBar` composable
  - Shows "X/Y today · X/Y in queue"
  - Warning color when at limit
- [ ] **6.6** Update `SearchScreen` to show external results when in external mode
- [ ] **6.7** Add `DownloadLimitsBar` above external results list
- [ ] **6.8** Load external image URLs with Coil (already used in app)

### Phase 7: Request Functionality

- [ ] **7.1** Add `requestAlbumDownload(result)` action to `SearchScreenActions`
- [ ] **7.2** Implement `requestAlbumDownload` in `SearchScreenViewModel`
  - Call RequestAlbumDownloadUseCase
  - Update result's inQueue status on success
  - Refresh limits after request
- [ ] **7.3** Add `onExternalResultClick(result)` action
- [ ] **7.4** Implement navigation for "In Catalog" items
  - If result.inCatalog && result.catalogId != null → navigate to album/artist screen
- [ ] **7.5** Wire Request button in result cards to requestAlbumDownload
- [ ] **7.6** Disable Request button when limits.canRequest is false
- [ ] **7.7** Show loading state on Request button while request is in progress
- [ ] **7.8** Handle request errors (show snackbar/toast)

### Phase 8: My Requests Screen

- [ ] **8.1** Create `MyRequestsScreenState` data class
  - requests: List<DownloadRequest>?, isLoading, error, limits
- [ ] **8.2** Create `MyRequestsScreenActions` interface
  - refresh(), onRequestClick(request)
- [ ] **8.3** Create `MyRequestsScreenViewModel`
  - Load requests and limits on init
  - Pull-to-refresh support
- [ ] **8.4** Create `MyRequestsScreen` composable
  - Header with limits display
  - Sections: Pending, Completed, Failed
- [ ] **8.5** Create `PendingRequestItem` composable
  - Shows album name, artist, queue position, progress bar
- [ ] **8.6** Create `CompletedRequestItem` composable
  - Shows album info, clickable to navigate to catalog
- [ ] **8.7** Create `FailedRequestItem` composable
  - Shows album info and error message
- [ ] **8.8** Add navigation route for MyRequestsScreen
- [ ] **8.9** Add "My Requests" entry to Profile screen
  - Only visible when hasRequestContentPermission
- [ ] **8.10** Wire navigation from Profile to MyRequestsScreen
- [ ] **8.11** Add Interactor for MyRequestsViewModel in InteractorsModule

---

## Decisions (from discussion)

1. **Toggle UI placement**: Inside SearchBar trailing area (compact toggle/icon). Only visible when user has RequestContent permission AND external search setting is enabled.

2. **"In Catalog" items**: Tapping navigates to the catalog item (album/artist screen).

3. **Filter chips**: Add filter chips to search screen for BOTH catalog and external search modes.
   - Catalog mode: Album, Artist, Track filters
   - External mode: Album, Artist filters (no Track - external API doesn't support it)

4. **Offline handling**: Show error (same as catalog search - both require network).

5. **My Requests location**: Entry point in Profile screen.

6. **Toggle state persistence**: Persist locally like other user settings that don't sync with server (local preferences only).
