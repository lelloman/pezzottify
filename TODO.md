## [pezzottify-server]

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

