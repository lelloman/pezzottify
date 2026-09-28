# Android Works implementation plan

Status: implemented, 2026-09-28. Automated validation is recorded below.

## Outcome and scope

Bring the existing web Works experience to the Android phone app: discover a
composition from a track or search, browse its recordings and relationships,
and play its versions. Use the current server contracts documented in
[Works v1](works-v1.md); no new server endpoints are expected.

Deliver these surfaces together, in the implementation order below. Work likes,
Work radio seeds, editing, offline Work-page storage, Android TV layouts, and
the standalone local-player app are outside this first delivery. A Work is a
composition; playback still operates on track IDs.

## Current code and integration points

- `domain/remoteapi/response/TrackResponse.kt`, domain `statics/Track.kt`,
  localdata's Room `statics/model/Track.kt`, and UI `content/Track.kt` currently
  carry track enrichment but omit Work resolution and its status.
- `ui/screen/main/content/track/TrackScreen.kt` renders `profile.workTitle`
  as plain metadata. A resolved Work must also be visible when that profile
  is absent.
- `ui/Navigation.kt` defines typed destinations; `ui/screen/main/MainScreen.kt`
  hosts the phone navigation graph.
- `SearchScreenViewModel` debounces queries by 400 ms and selects catalog or
  streaming search. Works use a separate endpoint and need independent state
  in either mode.
- `app/.../ui/InteractorsModule.kt` wires screen interactors and existing
  playback actions. `RadioCreationController` handles cancellable queue creation.
- Web references: `Work.vue`, `WorkResult.vue`, `SearchWrapper.vue`, and
  `Track.vue`; `store/remote.js` and `store/playback.js` implement all-versions
  pagination and playback context.

Paths above are relative to `android/` unless identified as web references.

## 1. Data contract and track cache

- Add serializable Work summary/detail, resolution, relationship, recording,
  and paginated response models in `domain`. Match actual server JSON,
  including nullable metadata and unknown relationship labels.
- Add `searchWorks(query, limit)` and `getWork(id, limit, offset, scope)` to
  `RemoteApiClient` and its implementation, with domain use cases following
  `GetGenreTracks` conventions. Reuse existing authentication and error handling.
- Decode `work_resolution` and `work_enrichment_status` on track responses;
  preserve them through domain mapping, Room caching, content resolution, and
  UI models. Missing fields remain valid for older responses.
- Add nullable serialized columns and converters to the track cache, with an
  incremental Room migration and exported schema. Preserve existing cached
  content. Verify that existing cache refresh rules eventually refresh tracks
  cached before this feature; do not leave their missing Work links permanent.
- Fetch Work pages on demand with screen-scoped state. Do not introduce a
  persistent Work catalog for this delivery. Clear transient data on account
  or server changes and follow existing request cancellation behavior.

Acceptance: a track with a Work link and no track enrichment profile retains
that link through an API-to-cache-to-UI round trip. Old cached tracks and older
responses still load.

## 2. Work page and track entry point

- Add `Screen.Main.Work(workId)`, `toWork`, and a Work screen, ViewModel,
  state, actions/events, and interactor binding using existing screen patterns.
- Make the track's resolved Work title clickable. Prefer its canonical title;
  retain the current plain-text profile title when no resolved ID exists.
- Use a scrolling phone layout: back navigation; title, kind, creator portraits
  and names, composition year/range; relationships; recordings; expandable
  catalog identifiers and source links. Use catalog artist IDs for portraits
  and artist navigation, with a fallback when unavailable.
- Show ordered parts, parent links, and directed related-work links in
  expandable sections. Navigate to another Work page on tap. Render server
  relationships without performing client-side recursive graph traversal.
- Provide a recordings scope selector: all, this Work and its parts, related
  Works only. Default to all, matching web. Show album/performer context and
  part/related labels on recording rows; support track playback and navigation
  to track, album, and recording Work details.
- Use server `next_offset`/`has_more` for lazy pagination. An empty resolved
  page can still have more data. Reject a non-advancing cursor, deduplicate
  recordings by track ID, and cancel/discard stale pages after scope changes.
- Distinguish initial loading, missing Work, empty recordings, and network
  failure. A later-page failure keeps loaded content and offers retry. Keep
  scope and scroll position on ordinary back navigation.
- Add English/Italian strings, accessible labels, touch targets, and large-font
  checks using the existing Android theme and components.

Acceptance: track → Work → related Work → back works; parts and related
recordings remain distinguishable; unavailable recordings cannot start playback;
missing portraits or dates do not leave misleading placeholders.

## 3. Works in search

- Run Work search alongside the selected catalog/streaming path after the
  existing debounce. Give it separate loading, results, empty, and retry states;
  a Work-search failure must not suppress other results and vice versa.
- Cancel both requests when the query changes or clears, and prevent late
  responses from replacing newer results. Reset expansion for a new query.
- Show a separate Works section above catalog results in both modes, with six
  initial rows and expansion to the returned limit of 25, matching web. Rows
  show title, creators, available year, and creator portraits/fallback.
- Keep existing artist/album/track filters scoped to catalog results; the
  separate Works section remains visible for nonempty queries. No new filter
  chip or change to the server's catalog/SSE search enums is needed.
- Tapping a result opens the Work page. Work entries in persisted search
  history/recently viewed are deferred because those contracts currently cover
  other content types; do not record a Work UUID as a track or artist.

Acceptance: Works appear with smart/streaming search enabled and disabled;
rapid query changes, query clearing, catalog filters, and independent failures
behave consistently.

## 4. Play all versions

- Add the track-page action when a resolved Work exists. Fetch every Work
  recordings page with `scope=all`, matching the current web action even though
  that scope includes parts and related Works. Reuse a domain paging helper.
- Apply Android's existing availability rules, deduplicate IDs, and place the
  initiating track first if playable and present. Show progress and allow
  cancellation; disable duplicate submissions while loading.
- Build the entire queue before replacing playback through
  `RadioCreationController` and the existing player/router. Empty results,
  request failure, or cancellation leave current playback intact.
- Match the web context: `source=work_versions`, track seed ID/type, Work title,
  `settings.work_id`, count, and continuation initialized from the fetched IDs.
  Audit existing continuation behavior against web instead of assuming this
  source should behave like `greatest_hits`.
- Add a clear “Versions of …” queue/player label; the current Android label
  mapping only special-cases greatest hits. Verify restoration, queue edits,
  local playback, and remote session commands preserve the Work context.
- The Work page itself supports individual recording playback in this delivery;
  the all-versions action remains on the track page, matching the current web UI.

Acceptance: a multi-page Work plays all eligible unique recordings with the
selected version first; Android can send and receive this queue through remote
playback, and a restart preserves its context.

## Verification and delivery

Implement in four reviewable slices corresponding to the sections above, then
run a combined phone acceptance pass before considering the feature complete.

- Contract tests: real server-shaped fixtures for Work search/detail and track
  resolution; absent/null metadata, composition ranges, relationship direction,
  and a page with zero resolved tracks but an advancing cursor.
- Cache tests: migration preserves existing tracks; Work resolution survives
  serialization and mapping; tracks without enrichment still expose Work links.
- ViewModel tests: independent searches and cancellation, Work load/retry,
  pagination and scope-change races, and back-stack state where supported.
- Playback tests: all-page loading, deduplication, unavailable seed, empty/error
  and cancellation preserving playback, concurrent playback requests, context
  persistence, and remote command round trips.
- Build the Android app and run affected module tests and lint. Manually check
  a simple song, a multi-part composition, a related-work graph, and missing
  metadata on a phone/emulator, including both search modes and remote playback.
- Update `works-v1.md` to describe Android support only after implementation and
  validation are complete. Keep this plan's status accurate as slices land.

Implementation validation:

- 573 tests passed across domain, UI, localdata, and remoteapi, including Work
  contracts, search cancellation and independent errors, paging and scope races,
  playback cancellation and ordering, cache migration/round trips, remote command
  reception, and playback restoration.
- Phone debug APK build and phone debug lint passed using the clean assistant
  checkout at the revision pinned by `simple-android-assistant.rev`. The default
  sibling checkout was older than the pin and could not compile the existing
  assistant diagnostics code.
- Device smoke test could not be completed: the available test AVD remained
  `RUNNING_LOCKED` after normal and cold boots and could not launch the installed
  APK. Live phone navigation, large-font appearance, and two-device playback
  still need a device acceptance pass; unit tests cover their state/command
  behavior but do not replace that check.

