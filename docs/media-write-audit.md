# Media publication, removal and presence ownership

Ticket: [LLPR/PEZZOTTIFY-6](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/6)

Implementation audit: 2026-09-05, following the read boundary in story #5.
Paths below are relative to `pezzottify-server/src`.

## Operation inventory

| Consumer | Previous behavior | Current boundary |
| --- | --- | --- |
| `ingestion/conversion.rs` | Copy or ffmpeg output directly into published storage; attachment failures could still complete the job. | `MediaManager::stage`/`commit` for conversion, `publish_file` for copies. Only a publication receipt marks a file converted. Failure leaves the job retryable. |
| `media/track_materializer.rs` | Backend separately staged and published proxy audio, then updated catalog/search/proxy records. | Streaming producer fills a manager staging lease and commits it with proxy provenance. Progressive playback remains independent of publication. |
| `media/mod.rs::read_image` | Remote-image cache used a separate atomic-write helper. | Validated image bytes use the same publication journal. Cache persistence remains best effort for an otherwise successful image response. |
| `background_jobs/jobs/proxy_retention.rs` | Prefix-based file deletion, with separate catalog and search mutations. | Resolve the exact proxy revision, then `retain_copy` or `remove_copy`. Existing 48-hour and 50%-listening rules remain. |
| `catalog_store/sqlite_queries.rs`, `sqlite_catalog_adapter.rs` | Query enrichment, setters and availability reconciliation opened or probed media files. | Catalog queries and transactions only handle metadata. `MediaCatalogView` enriches API reads with live presence after underlying queries return. Manager reconciliation supplies conditional observations. |
| `background_jobs/jobs/catalog_availability_stats.rs` | Filesystem scan inside the catalog write transaction. | Manager recovery, indexed presence pages and conditional catalog repair, then derived statistics. |
| `download_manager/watchdog.rs`, `background_jobs/jobs/album_embedding_sync.rs` | Direct physical existence/open checks. | Shared media presence probe; only confirmed absence authorizes missing-file handling. Track embedding already uses the manager-owned reader from #5. |
| `server/metrics.rs`, `server/storage_report.rs` | Independent recursive media directory accounting. | Shared media accounting helper. Other database and ingestion-staging accounting stays with its existing owner. |

Published audio and catalog images are covered together. Upload/ZIP staging, audio
analysis, matching, conversion decisions, notifications and download-request workflow
remain ingestion responsibilities. Database entity deletion remains metadata deletion;
it does not acquire physical cascade semantics. Existing image CRUD endpoints are not
implemented by this story.

## Publication and recovery

`media/mutations.rs` owns a local operation journal under `.media/pending`, staging
leases under `.media/staging`, copy receipts under `.media/copies`, and image pointers
under `.media/images`. Published generations live in `audio/.managed` or
`images/.managed`. Each generation has an immutable UUID revision and provenance:
ingested, proxy (with materialization timestamp), image cache, or retained.

A lease captures the previous reference. Commit validates a complete, nonempty staged
file (and image type where applicable), syncs it, persists publication intent, exposes
the immutable generation, and conditionally attaches its reference. A conflicting
attachment cannot replace a newer copy. Dropping an uncommitted lease cleans staging;
recovery cleans abandoned leases from a previous process epoch.

The receipt distinguishes committed publication from pending secondary effects.
Search and proxy-scheduling failures leave durable work for replay without requiring
another publication. Search publication is idempotent. Recovery handles interruption
before or after exposure, attachment and removal, and rejects journal paths that do
not match the managed identity. Startup and availability maintenance retry pending
work. Recovery rotates through at most 1,000 records per invocation and checks
cancellation; directory enumeration itself is not limited to 1,000 names.

This local backend assumes one writer process per configured media root. Manager
instances within that process share a mutation lock. Managed directories must be real
directories, not symlinks, and the media root is trusted server-owned storage. This
does not introduce distributed locking or federation.

## Removal and compatibility

Removal first conditionally detaches the exact current revision, then updates derived
state and unlinks only that revision. Repeated removal succeeds without deleting a
replacement. Ingested, retained and unclassified legacy media are protected from
automatic proxy retention. Legacy proxy scheduling entries without an unambiguous
managed receipt are retired conservatively, leaving their bytes intact.

Open readers own their file descriptors. On Unix they can finish after unlink; where
unlink is refused, the journal retains cleanup work for retry. Previous generations
are retained on replacement. Generic garbage collection, orphan reclamation and
storage quotas belong to later policy work; this story does not delete old files by
guessing ownership.

## Presence and catalog transactions

Presence is `Present`, `Missing`, or `Unknown(error)`. Probing does not download media.
Unknown observations never authorize destructive repair. Existing API availability
remains a binary projection; the richer observation is internal to media maintenance.
The legacy locator setter validates syntax without opening the file.

Reconciliation reads available tracks in indexed pages of up to 1,000, probes outside
catalog transactions, journals confirmed missing observations, and applies updates
only when the observed reference still matches. Cancelled or stale observations do
not overwrite newer attachments. CatalogStore retains references, persisted flags,
derived album/artist availability and aggregate statistics, but no physical media IO.

## Regression coverage

`media/mutation_tests.rs` covers replacement with an active reader, failed staging,
stale publication and retention, idempotent removal, interrupted publication/removal,
protected legacy and retained copies, conditional presence repair, unknown symlink
observations, cancellation, injected SQLite attachment failure, secondary-effect
retry, actual ingestion retry, abandoned staging, Unicode identity and forged paths.
Existing media tests exercise image persistence failure and progressive retrieval;
the search regression replays publication before removing its availability document.
The heavy-job integration verifies missing-file reconciliation and persisted snapshots.

Explicit vault/adapters contracts remain story #7. Multiple locations, read-through
caching, configurable eviction, federation and database response caching are not part
of this change.
