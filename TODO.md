## [pezzottify-server]

### Completed

- **Skeleton sync removed** - Dead code removed (`src/skeleton/`, `src/server/skeleton.rs`, `tests/e2e_skeleton_tests.rs`).
  Android now uses on-demand discography API (`/v1/content/artist/{id}/discography`) instead.
- **Related Artists enrichment** - Implemented via Last.fm + MusicBrainz API integration.
  Two-phase background job implementation with MusicBrainz mbid storage and Last.fm similar artists lookup.
  See `docs/related-artists-design.md` for details.
- **Composite Popularity Scoring** - Fully implemented.
  Combines listening data (70%), impressions (25%), and Spotify popularity (5%) with weight redistribution.
  Includes `item_impressions` table, `POST /v1/user/impression` endpoint, and updated `PopularContentJob`.
  See `docs/composite-popularity-scoring.md` for details.
- **E2E Testing Infrastructure** - Fully implemented.
  Docker Compose-based E2E testing with Python pytest test runner.
  Coordinates pezzottify-server, LelloAuth OIDC provider, web clients (Playwright), and Android emulators (ADB).
  See `docs/e2e-testing-implementation-plan.md` for details.

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

