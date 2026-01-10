## [catalog-server]

### Completed

- **Skeleton sync removed** - Dead code removed (`src/skeleton/`, `src/server/skeleton.rs`, `tests/e2e_skeleton_tests.rs`).
  Android now uses on-demand discography API (`/v1/content/artist/{id}/discography`) instead.

## [android]

### Breaking changes from Spotify migration

- **Skeleton sync cleanup** - Android still has skeleton-related code that needs cleanup:
  - Rename `domain/.../skeleton/` → `discography/` (it's a discography cache, not skeleton sync)
  - Rename `SkeletonStore` → `DiscographyCache` (better naming)
  - Remove unused skeleton sync API methods from `RemoteApiClient`:
    - `getSkeletonVersion()`
    - `getFullSkeleton()`
    - `getSkeletonDelta()`
  - Keep `DiscographyCacheFetcher` - it correctly uses regular discography API

Note: The Android `SkeletonStore` is actually a discography pagination cache, NOT skeleton sync.
It stores album IDs per artist for efficient pagination. The naming is just confusing.

## [web]

- Implement a Toast/Snackbar like messages visualization component
- Make right-click contextual menu for albums and artists (after user playlists)
- Make titles and texts on single line and sliding if too long
- Use a logger instead console.logging all over the place
- Implement User page (profile?)
- Add a logo with a home link
- Implement track selection (instead of play command), so that group of tracks can be added/removed
- Implement playlist reordering from playlist content page
- Add save/cancel button and show edited state in currently playing side bar
