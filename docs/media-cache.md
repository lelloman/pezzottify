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
read from the catalog database, not from the media disk. Legacy and unmanaged locators
(`audio/track.mp3`, hashed or imported paths without a publication revision) are not
cacheable; they bypass the cache and continue to be read in place. This is the legacy
validity rule.

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

Audio keys make stale entries unreachable even if best-effort invalidation did not
complete. Invalidation is cleanup, not the correctness boundary. Recovery replays the
same ordering for interrupted publications and removals.

Readers hold descriptors. Invalidation never waits for or interrupts a reader, and a
later physical unlink cannot change bytes already exposed through an open descriptor.
Concurrent fills are a #12 concern; the contract requires them to publish a complete
object atomically, never overwrite an object a reader may hold, deduplicate fills per
key, and refuse to publish an entry whose key was invalidated during the fill. A fill
must not be able to resurrect an invalidated media entry without a fresh authoritative
read.

## Restart and external changes

Audio entries can survive a restart: their version is the durable managed revision
from the catalog. An entry whose key does not match the current catalog reference is
ignored. Image entries are process-scoped as described above, so a restart starts
cold for images and falls back safely.

Pezzottify-made changes are reflected through the publication, replacement and
deletion paths. Arbitrary files changed outside Pezzottify cannot be detected
instantly without monitoring or probing: if an external process rewrites an
authoritative file in place without changing the managed catalog reference, audio
entries for that reference may serve the old bytes until the media is republished or
the server restarts. This story deliberately does not add per-hit original-storage
validation to hide that limitation.

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

[[media_cache.caches]]
name = "nvme"
path = "/mnt/nvme/pezzottify-media"
max_bytes = 1073741824
```

Configuration is read at startup. A deployment without a `[[media_cache.caches]]`
entry behaves exactly as before. Cache directory names must be unique, budgets must
be positive, and directories may not overlap media storage or each other (compared
after canonicalization). Startup fails on an invalid layout rather than risking a
cache that could delete authoritative files.

Each cache keeps complete objects under `<path>/objects/<kind>/<id-hash>/`; lookup
opens only that directory. A fill creates a temporary file, copies and flushes it,
and renames it into place, so a partial read or interrupted write is never visible.
The byte budget is enforced by tracking persisted usage at startup plus reservations
for in-flight fills. Fills skip when the object is already present, when another fill
for the same key is in flight, when no cache has room, or when the object is larger
than every budget. Full caches therefore fall back to authoritative storage instead
of evicting; eviction and richer diagnostics belong to #13.

Population runs on a dedicated bounded filesystem pool, after the response has
started. A slow or failing cache write therefore never gates streaming, and a failed
fill leaves no entry behind. Invalidation increments a per-media epoch and removes
the media directory before the new authoritative state is visible; a fill that
started under an older epoch refuses to publish. Audio objects survive restart
because their keys embed the durable managed revision. Image objects are
process-scoped, so startup removes them and images start cold. Legacy and unmanaged
audio locators continue to bypass the cache entirely.
