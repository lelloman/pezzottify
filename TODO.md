## [catalog-server]

### Disabled features (Spotify schema - large catalog)

- **Skeleton sync** - Disabled because it would require downloading indices of 350M+ tracks.
  The skeleton API was designed for smaller self-hosted catalogs where clients could cache
  the entire catalog structure locally. Files: `src/skeleton/`, `src/server/skeleton.rs`,
  `tests/e2e_skeleton_tests.rs`

## [android]

### Breaking changes from Spotify migration

- **Skeleton removal** - Android heavily relies on skeleton sync for local catalog caching.
  With skeleton disabled on the server, Android needs to be updated to work without it.
  Affected files:
  - `domain/.../skeleton/CatalogSkeletonSyncer.kt` - skeleton sync logic
  - `domain/.../skeleton/SkeletonStore.kt` - local storage interface
  - `localdata/.../skeleton/` - Room database implementation
  - `domain/.../remoteapi/response/SkeletonResponses.kt` - API response models
  - Various usages in login, logout, settings, app initialization

  **Options:**
  1. Remove skeleton entirely and rely on on-demand fetching
  2. Implement a lighter-weight local search index (if needed)

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
