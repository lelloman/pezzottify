# Media read interception and cache validity

Ticket: [LLPR/PEZZOTTIFY-8](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/8).
This builds on the read/write audits and the vault contracts in
[media-read-audit.md](media-read-audit.md), [media-write-audit.md](media-write-audit.md)
and [media-vaults.md](media-vaults.md). Production cache directories and population
policy belong to #12; this story defines the interception and validity contract.

## Interception points

`MediaManager` owns the only server-side payload reads for published audio and catalog
images. An optional `MediaCache` is installed at startup and checked before the
authoritative adapter or remote source is opened.

| Consumer | Path | Intercepted read |
| --- | --- | --- |
| Audio playback and progressive ranges | `server/stream_track.rs` through `MediaManager::lookup_audio` | Whole cached file, then manager-owned seek and stream |
| Background embedding and other indirect file consumers | `MediaManager::open_local_audio_blocking` | Whole cached file exposed as an owned descriptor |
| Images | `server/handlers_catalog.rs::get_image` through `MediaManager::read_image` | Cached bytes validated with the same image-type check as a local file |
| Proxy fallback | `lookup_audio` miss, then existing permission and preference gates | Unchanged; a cache miss never enables proxy access on its own |

Unpublished ingestion staging remains the documented path exception: uploaded ZIPs,
ffprobe/ffmpeg inputs and conversion outputs are not published catalog media and stay
with ingestion. Availability reporting, storage accounting, publication, removal and
retention keep their existing ownership. In particular, a cache hit does not change
availability semantics: presence answers whether authoritative storage owns a copy,
not whether a fast copy exists. Those projection paths may still probe authoritative
storage, but they are not payload retrieval.

With no cache installed, every path behaves exactly as before the story. Lookups also
fall back to authoritative storage when the cache misses, errors, is unreachable,
returns an invalid image or lacks the required capability.

## Keys

A cache object is immutable and identified by `CacheKey`:

- media identity: media kind plus catalog identity (`MediaKey`),
- version: the authoritative token the bytes were derived from,
- generation: how long the token remains meaningful.

Audio keys use the managed authoritative locator
(`audio/.managed/{id}.{revision}.{ext}`) as their version. The revision is an immutable
publication UUID, so replacement and deletion cannot reuse an old key. The version is
read from the catalog database, not from the media disk. Existing locators without
a publication revision are also cacheable: their key uses the catalog path and a
process generation. They fill on first access, serve subsequent reads from cache,
and are discarded on restart. No migration or special filename is required.

Image keys use the catalog source URL for the item. The catalog database is the fast
metadata source; the image pointer under `.media/images` is not consulted for a hit.
Because a same-URL re-publication would otherwise reuse the key, image entries are
also scoped to the server process generation. Persisted image entries from an earlier
process are ignored and fall back to authoritative storage until a later story
provides durable per-publication image versions. Images with no catalog source URL
(for example purely uploaded artwork) are not cacheable and keep their current local
path.

A cache implementation must keep all hit metadata (key, size, content type and object
location) on its own fast storage. A successful lookup never opens, stats or probes
authoritative media storage, and a miss or error is the only reason to touch it.

## Invalidation and ordering

Authoritative publication, replacement and deletion call `MediaCache::invalidate`
for the affected media before the new authoritative state becomes visible, under the
existing media mutation lock:

1. Replacement: old entries are hidden or removed while the previous revision is
   still authoritative; the new file is exposed and the catalog reference/pointer is
   switched afterwards.
2. Deletion: entries are invalidated before the catalog reference or image pointer is
   detached and before the file is unlinked.
3. Publication of an image from the same source URL relies on this ordering; an
   in-flight reader either opened the old object while it was authoritative, or must
   miss the cache after invalidation and re-read the new authoritative state.

Managed audio revisions make old keys unreachable after the catalog reference
changes. For every entry, invalidation advances its epoch before best-effort file
cleanup, so failed cleanup cannot make invalidated bytes reachable again. Recovery
replays the same ordering for interrupted publications and removals.

Readers hold descriptors. Invalidation never waits for or interrupts a reader, and a
later physical unlink cannot change bytes already exposed through an open descriptor.
Concurrent fills are a #12 concern; the contract requires them to publish a complete
object atomically, never overwrite an object a reader may hold, deduplicate fills per
key, and refuse to publish an entry whose key was invalidated during the fill. A fill
must not be able to resurrect an invalidated media entry without a fresh authoritative
read.

## Restart and external changes

Managed audio entries can survive a restart: their version is the durable managed revision
from the catalog. An entry whose key does not match the current catalog reference is
ignored. Existing audio paths without a durable revision and image entries are
process-scoped, so a restart discards those copies and refills them on demand.

Pezzottify-made changes are reflected through the publication, replacement and
deletion paths. Arbitrary files changed outside Pezzottify cannot be detected
instantly without monitoring or probing: if an external process rewrites an
authoritative file in place without changing the catalog reference, cached entries
may serve old bytes until the media is republished or the disposable cache is
cleared. For audio without a durable revision, restarting the server also discards
the old copy. Cache hits do not probe original storage to detect external changes.

## Failure behavior

Cache lookup, read and invalidation errors are logged with the media identity and are
never allowed to fail an otherwise valid read. Cache work runs on the existing
bounded filesystem pools; it does not hold catalog transactions or the media mutation
lock while serving bytes. No cache state is cached in a way that a panic or a failed
fill can poison later reads: the manager consults the cache on each lookup and falls
back on any error.

## Filesystem caches (#12)

Operators configure one or more ordered cache directories. Order is lookup and
population preference; each directory has its own byte budget.

```toml
[[media_cache.caches]]
name = "ssd"
path = "/var/cache/pezzottify/media"
max_bytes = 53687091200
lru = true # optional; false (the default) skips population when full

[[media_cache.caches]]
name = "nvme"
path = "/mnt/nvme/pezzottify-media"
max_bytes = 1073741824
```

Configuration is read at startup. A deployment without a `[[media_cache.caches]]`
entry behaves exactly as before. Cache directory names must be unique, budgets must
be positive, and directories may not overlap media storage or each other (compared
after canonicalization). The cache's internal `objects` directory cannot be a
symlink, so recovery cannot follow it into original storage. Startup fails on an
invalid layout rather than risking a cache that could delete authoritative files.

Each cache keeps complete objects under
`<path>/objects/<kind>/<id-hash>/<epoch>/`; lookup opens only the current epoch
directory. A fill creates a temporary file, copies and flushes it, and renames it
into place, so a partial read or interrupted write is never visible. The byte budget
is enforced by tracking persisted usage at startup plus reservations for admitted
fills. An in-memory ledger charges each allocated temporary path before writing,
then moves that exact charge to the destination before rename. Invalidation releases
only ledger charges for successfully removed epochs; it never subtracts a measured
directory size from committed usage. Retained temporary or complete files after a
cleanup failure remain conservatively charged until cleanup or startup recovery.
All reservation transfers and ledger updates are ordered with invalidation under
the short policy lock; filesystem operations remain outside it. Fills skip when the object is already present, when another fill for the same
key is in flight, when no cache has room, or when the object is larger than every
budget. With the default `lru = false`, full caches skip population. Optional
eviction and diagnostics are described below.

Population runs on a dedicated bounded filesystem pool, after the response has
started. Admission is nonblocking and bounds both pending fills and retained payload
bytes before an image copy is made; a saturated queue simply skips population. A
slow or failing cache write therefore never gates streaming. The shared policy lock
only guards in-memory accounting and the epoch check: source reads, temporary-file
writes, renames, directory fsyncs and invalidation cleanup all run outside it, so
foreground lookups never wait on cache I/O. A failed fill removes its temporary file
and releases its reservation and deduplication marker, including when the source
panics. Pending-key and retained-payload admission remains with the blocking worker
until it releases its request, even if its async waiter times out or is cancelled.

Invalidation increments a per-media epoch under the lock before any cleanup; lookup
captures the current epoch before opening and rechecks it after obtaining the
descriptor, so old entries become unreachable even if the filesystem cleanup fails
or invalidation races with the open. Cleanup then removes stale epoch directories
without holding the lock. A fill that started under an older epoch refuses to publish.

At startup, the manager pages through persisted catalog audio references without
presence enrichment or original-media access, regardless of observed availability. Recovery matches each audio object's
key hash to the current durable catalog revision, moves complete length-tagged matches from any
numeric epoch (or the earlier flat layout) into epoch zero, and removes obsolete
revisions, unknown layouts, incomplete files and trash before rebuilding the exact
allocation ledger. Epoch numbers are process-local ordering tokens, not evidence of
validity: recovery never chooses the largest epoch. If a mutation changed or deleted
the catalog reference while cleanup failed, the old key cannot match and its bytes
are reclaimed. A crash during normalization leaves complete revision-keyed files
that the same reconciliation can safely process again. Recovery errors abort cache
installation with cache/path context instead of installing uncertain accounting.
The resulting full cache can immediately serve unchanged managed audio after restart,
without opening original media. Images and audio without durable revisions are
process-scoped: startup removes them and the next access refills them.


## Eviction, integrity and diagnostics (#13)

Set `lru = true` on a cache directory to opt into eviction. Lookup still follows
configuration order; population first uses existing free space. Only when no cache
has room does a background fill attempt eviction, considering the least recently
used eligible complete objects across enabled caches. Oversized fills never evict.
Hits and completed fills update in-memory recency. Restart resets recency, with
recovered objects tied in deterministic pathname order. Policy changes take effect
only at startup. Configure a directory for one server writer; sharing directories
between running servers is unsupported.

Eviction claims identities under the short policy mutex, then performs all file I/O
outside it. It excludes in-flight fills and stale epochs. Readers hold shared
nonblocking OS file locks on their descriptors. Lookup first claims its media identity
under the policy mutex, before any filesystem access; eviction skips pending
lookup claims. Lookups encountering an existing eviction claim return a miss.
The lookup claim is released on success, error or panic, with the shared descriptor
lock taking over protection for a returned reader. This closes the interval between
opening a descriptor and locking it without holding the policy mutex during I/O.
Eviction tries an exclusive lock
and skips held readers, so their bytes remain readable **and charged**. It never
waits for a reader. A failed unlink retains the allocation and can be retried; a
concurrent invalidation owns settlement of its old epoch. In particular, files
moved to trash after failed cleanup remain charged: their absent former paths are
not evidence that disk space was freed. Trash is reclaimed on restart. A fill
skips when locked readers or retained trash leave insufficient capacity. Lookup
reconciles missing complete files even when LRU is disabled, restoring capacity
after cache loss. It checks the allocation's recency incarnation and excludes
active fills, eviction claims and changed invalidation epochs, so a missing-file
probe cannot release a newer refill or bytes retained in trash.

Complete filenames record the expected byte length (`<key-hash>.len<bytes>.<extension>`).
Lookup checks regular-file metadata and length without reading payload bytes.
Cache fills validate the copied length and publish atomically after flush/sync;
no payload hashing is performed during population or lookup. Key hashes only
encode identity/version information into safe filenames. Freshness still comes
from catalog versions and invalidation epochs, independently of byte integrity.
Truncated or unreadable entries fall back to existing retrieval. Same-length
external corruption is not detected: cache files rely on normal filesystem
integrity, like original media. They must not be modified externally.

Old checksum-named entries are discarded during startup and repopulated on demand;
original media and catalog references are unaffected. Restart recovery preserves
matching length-tagged durable audio and discards process-scoped audio and images,
partial files and obsolete revisions. External modification of originals still has the
limitation documented above.

The existing Prometheus endpoint exposes:

| Metric | Labels / interpretation |
| --- | --- |
| `pezzottify_media_cache_events_total` | `cache`, `outcome`: audio/image `hit`, `miss` and `fallback_read` outcomes use `cache="all"`; fallback counts attempts to use the existing retrieval path, including failed reads |
| Same counter | `fill_completed`, `fill_failed`, `eviction`, `eviction_busy`, `eviction_failed`, `missing_reclaimed` have the configured cache name; failure includes unwinding fills |
| Same counter | `fill_skipped_empty`, `fill_skipped_present`, `fill_skipped_invalidated`, `fill_skipped_duplicate`, `fill_skipped_capacity`, `fill_skipped_admission`, `fill_skipped_runtime`, `fill_skipped_worker` use `cache="all"`; worker timeout can precede a later completion because the bounded worker retains its admission |
| `pezzottify_media_cache_bytes` | `cache`, `state`: `allocated` includes conservatively charged temporary/trash files, `reserved` covers admitted fills before allocation, `budget` is the configured maximum |

Cache read/lookup errors increment `audio_error` or `image_error`. Image errors
also count as image misses; audio lookup errors are separate from audio misses. Existing audio paths
participate in the same hit/miss counts as managed audio. Internal fill
deduplication lookups do not count as consumer hits. Cache
failures log identity and error; eviction failures also name the cache. Metrics
labels never include media identities or paths.

### Reproducing original-storage avoidance

Run `cargo test --features fast --lib representative_audio_seek_artwork_workload
-- --nocapture` in `pezzottify-server/`. It reports cold/warm elapsed time for a
4 MiB audio payload, sixteen 4 KiB seeks (both cache-disabled baseline and warm)
and fixture JPEG artwork. Fills finish
before the warm phase; the entire original media root is replaced by a regular
file, rejecting original opens, stat/existence checks and image pointer resolution.
The test also calls audio metadata before streaming the warm descriptor.

On Linux, run `bash scripts/verify-media-cache.sh` from `pezzottify-server/`
(requires `strace` and Python 3). It traces pathname operations **and** descriptor
reads/metadata with resolved descriptor paths, counts original-media calls during
the cold phase including background population, and asserts zero during the warm
phase. Thus ignored original-media existence/stat probes also fail verification.
Markers exclude fixture preparation and teardown. This is a reproducible local
workload rather than a physical-HDD performance benchmark; recorded times depend
on the host, filesystem, tracing overhead and build profile.

The PR and ticket handoff record the measured cold/warm times and original-media
call counts for the reviewed workload. The Linux regression
`cache_open_and_small_range_do_not_scan_payload` uses per-thread kernel read
accounting to prove opening a 64 MiB cache object and reading a 4 KiB range does
not scan the payload. Target-storage benchmarking is still needed to quantify
the speedup on a physical HDD/SSD setup.

## Web admin visibility

Admin → Server → Media cache shows installed cache directories, their byte budgets,
allocated and reserved bytes, eviction policy, completed fills, evictions and errors.
Audio and artwork hit rates cover all caches together; individual cache hit rates
are not measured. Hit rate is hits divided by hits plus fallback attempts, and is
absent until the first request. Errors may also count as misses, so those columns
are not additive. Counters reset at server restart; allocation includes recovered
objects. Allocated bytes include temporary/trash files, and reservations cover
admitted fills before allocation.

`GET /v1/admin/media-cache` requires `ServerAdmin` and returns a non-cacheable JSON
snapshot from in-memory accounting and counters, without scanning cache directories
or original media. The panel refreshes every 15 seconds while visible, supports
manual refresh, and marks retained data as stale after a failed refresh. This is
read-only visibility; cache configuration remains in the server configuration file.
