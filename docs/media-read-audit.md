# Published media read boundary

Ticket: [LLPR/PEZZOTTIFY-5](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/5)

Audit date: 2026-09-05. Original source baseline: `8a3b57c9834080cf48e53e205f4aaeec8fb083c6`.

## Scope

`MediaManager` intercepts reads of published audio tracks and catalog images throughout the server. This includes local reads, remote retrieval, progressive playback, successor prefetch, fetch-then-persist behavior, and indirect file readers such as multipart embedding uploads. Audio and images are included together.

This first refactor preserves existing retrieval, authorization, range, validation and publication behavior. It does not add vault configuration, new caching policies, eviction, federation, or database response caching. Publication/deletion/availability ownership is story #6; explicit vaults/adapters are #7; multiple-vault resolution is #8; read-through caching is #12; eviction/diagnostics are #13.

All source paths below are relative to `pezzottify-server/src`.

## Read inventory and resulting interception points

| Entry point / consumer | Mechanism before refactor | Boundary after refactor |
| --- | --- | --- |
| `server/stream_track.rs::stream_track` | Catalog track lookup, root-confined file open, file metadata, seek and ReaderStream. | `MediaManager::lookup_audio` and owned `LocalAudio` metadata/range stream. HTTP headers and range parsing remain in handler. |
| Same handler, proxy fallback | Checks permission, user proxy preference and configured materializer, then starts or shares a progressive download. | Explicit `open_remote_audio` only after existing checks. `RemoteAudio` owns the in-flight reader and its memory lifetime. |
| Former `server/track_materializer.rs::download_and_publish` | Downloader audio stream, size validation, memory reservation, progressive chunks, atomic publication and catalog/search updates. | Moved unchanged into private `media/track_materializer.rs`, owned by MediaManager. |
| `track_materializer::schedule_successor` | Finds next unavailable track in album order and starts a prefetch. | Internal to the same manager-owned backend and single-flight map. It cannot create a separate HTTP/background download cache. |
| `server/handlers_catalog.rs::get_image`, local | Catalog image path, bounded filesystem-pool read, image type inference. | `MediaManager::read_image` returns validated bytes/type; handler maps errors to original responses. |
| Same image handler, remote | On local NotFound, resolves album/artist image URL, fetches bytes, validates type and attempts atomic persistence. | Entire retrieval and existing persistence side effect inside `read_image`. |
| `background_jobs/jobs/track_embedding_sync.rs::request_embedding` | Blocking multipart `Form.file` opens a raw path and reads it during upload. | Background job requests a local-only reader by track ID, then gives the owned descriptor to multipart. Filename, MIME inference and content length are retained; no unchecked reopen. |

`downloader/client.rs::open_track_audio` is the remote audio mechanism below the manager. Its only active production caller is the internal materializer. The whole-file audio/image download methods remain compatibility methods whose production callers are in the disabled legacy proxy module.

## Shared service and ownership

Production startup constructs the manager in JobContext and passes that same `Arc` to the HTTP server. JobContext clones retain the same manager. The HTTP-only `make_app` entry point creates its own manager for that application. Proxy initialization attaches the backend once, and both proxy status reporting and sync capability reporting query the manager.

The manager uses the shared database executor's catalog-read lane. Background local opens use its blocking submission API and background priority. Existing image work continues through bounded filesystem admission; HTTP administrative filesystem work shares that pool. The manager's image HTTP client retains the previous 30-second timeout.

Read operations use logical track or catalog-image IDs. Physical paths remain backend details. Owned local descriptors and progressive read handles establish reader lifetimes for later multi-location/caching work. Remote fetching is an explicit operation; local-only embedding reads never silently fetch missing content.

## Catalog probes and the temporary compatibility bridge

Catalog metadata reads can perform filesystem opens even without reading payload bytes:

- `catalog_store/sqlite_queries.rs::availability_from_audio_uri`: used by track and resolved album/track queries.
- `catalog_store/sqlite_catalog_adapter.rs::get_track_audio_path` and `open_track_audio_file`.
- Catalog reconciliation and `set_track_audio_uri` validate regular-file presence.
- `download_manager/watchdog.rs`: checks missing audio through the catalog compatibility opener; the redundant raw-path existence check has been removed.
- `background_jobs/jobs/album_embedding_sync.rs::complete_local_tracks`: uses the shared root-confined local opener. Album embeddings themselves read stored vectors, not audio bytes.

The safe opener and identifier validation now live in `media/local.rs`. CatalogStore temporarily calls this catalog-independent component. It accepts an already-resolved locator and never queries the database, avoiding a MediaManager -> CatalogStore -> MediaManager cycle or executor re-entry while holding a database connection. Existing path normalization, no-follow traversal, regular-file checks and platform-specific behavior are retained. Embedding readers and album probes now also receive this containment protection instead of following raw paths.

Logical availability and write ownership remain follow-up work. The catalog trait's path/open methods are compatibility backend APIs, not the API for new payload consumers.

## Explicit exceptions and non-payload access

### Unpublished ingestion staging

These reads stay inside ingestion because uploaded inputs do not yet have a published catalog media identity:

- `ingestion/file_handler.rs`: reads uploaded ZIP archives and extracted contents; creates/scans/cleans staging directories.
- `ingestion/analysis.rs`: reads staged inputs through ffprobe for media properties and tag extraction.
- `ingestion/fingerprint_workflow.rs`: both missing-duration fallbacks pass `temp_file_path` to `probe_audio_file`.
- `ingestion/converter.rs`: ffprobe/ffmpeg consume staged input paths.
- `ingestion/conversion.rs`: copies or converts staged input into published output. The publication side belongs to #6.
- Upload/mapping/analysis code also inspects staging file metadata and existence.

A future processing job reading an already-published track or image must use MediaManager; the exception is for unpublished staging inputs only.

### Storage accounting and mutations

- `background_jobs/jobs/proxy_retention.rs`: directory enumeration, file metadata and deletion.
- `server/metrics.rs` and `server/storage_report.rs`: directory walks and file sizes.
- Existing materializer, image persistence and ingestion publication writers remain operational. Their ownership is not redesigned in #5.

These are accounting/write/availability paths for #6, not unaccounted payload consumers. `get_album_embedding_coverage` accepts a media-path argument but its implementation does not read the filesystem.

### Dormant and unrelated paths

- `server/proxy.rs` is disabled in `server/mod.rs`; it is not an active fallback and its tests are not active regression coverage.
- `background_jobs/jobs/audio_analysis.rs` currently skips work and reads no audio.
- Frontend ServeDir/ServeFile assets are outside catalog media scope.
- Database/configuration/backup reads, JSON API responses, OIDC material, web/Android client caches and external downloader internals are outside this server payload boundary.

## Behavior and regression checks

Preserved contracts include full/suffix/open-ended/bounded audio ranges, clamping and invalid-range responses; missing tracks; proxy permission/preference/configuration gates; progressive delivery and shared downloads; size/memory limits, timeout/error propagation and prefetch; publication bookkeeping; and best-effort image persistence.

Image-specific details: invalid local image bytes still produce the existing 404 behavior without remote fallback. A local I/O error other than NotFound does not fetch remotely. Missing origin metadata produces 404; origin failure or invalid origin bytes produce 502. A valid origin response succeeds even if its cache write fails.

New `media/tests.rs` coverage exercises local ranges; missing-track distinction; safe reader ownership across path replacement; image miss/persist/hit; corrupt local images without fallback; failed/invalid upstream responses without persistence; cache-write failure; and shared progressive reads with cancellation and local publication. Existing materializer tests moved with the backend cover range waiting, failure propagation and stream accounting.

Existing regression suites to run:

- `tests/e2e_streaming_tests.rs`
- `tests/e2e_catalog_tests.rs`
- `tests/background_job_http_isolation_tests.rs`
- `tests/e2e_ingestion_tests.rs`
- Catalog safe-path and imported-URI unit tests, stream range-parser tests, and embedding job unit tests.

Final bypass review searches filesystem opens/reads, media path helpers, HTTP byte consumers, multipart file arguments, and ffprobe/ffmpeg input paths, then checks callers and module reachability. Compilation alone cannot establish complete interception.

## Validation result (2026-09-05)

- Full CI test command, `cargo test --features fast`: **1,289 passed, 0 failed, 34 ignored**. Debug symbols and incremental compilation were disabled through environment variables to reduce build disk usage; no repository build-profile changes were made.
- Default-feature focused suites: **112 passed** across media, catalog store, range parsing, embedding jobs, streaming/catalog/ingestion HTTP and background HTTP isolation.
- `cargo clippy -- -D warnings`: passed.
- `git diff --check`: passed.
- `cargo fmt --check`: reports pre-existing formatting differences only in the untouched `background_jobs/jobs/metadata_enrichment.rs`. That file was verified byte-for-byte against the baseline and excluded from this ticket.
