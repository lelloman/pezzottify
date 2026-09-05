# Media vault and adapter contracts

Ticket: [LLPR/PEZZOTTIFY-7](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/7).
Approved refinement: comment 178. This follows the read and write inventories in
[media-read-audit.md](media-read-audit.md) and [media-write-audit.md](media-write-audit.md).

## Ownership and identity

All published audio is authoritative, including proxy downloads. Proxy provenance
only influences automatic retention. Externally fetched images are cached with the
actual fetched URL as their source association. Images with unknown origins remain
protected. No files are relocated, and existing configuration remains valid.

`media/vault.rs` distinguishes media kind and catalog identity, content version,
representation digest, copy identity, vault identity and opaque adapter locator.
Each successful new publication gets a new content version and copy UUID; SHA-256
of the published bytes identifies its representation independently of location.
Thus identical bytes in separate publications have equal representation IDs and
separate versions/copy IDs. A transformed cached representation retains its source
content version and has its own representation digest. Publication UUIDs alone
must never be used to assert byte equivalence.

Copy descriptors are persisted in the existing publication journal and receipts.
Older receipts deserialize without a descriptor. Legacy files remain readable and
protected under the existing ownership rules; they are not accepted as a validated
cache source until publication establishes an explicit version. Migration does not
hash or rewrite the entire library. An unknown image source is not inferred from
its filename.

## Initial adapters

| Logical vault | Role | Adapter and capabilities |
| --- | --- | --- |
| `local-authoritative-audio` | Authoritative | Filesystem: read, range read, complete immutable publication, deletion, presence |
| `local-image-cache` | Cache | Same filesystem backend, existing image publication journal |
| `external-image-origin` | Authoritative source | HTTP image retrieval: read only |
| `proxy-audio-source` (when configured) | Authoritative source | Existing downloader: read only; acquired audio is published into local authoritative storage |

Vault role is independent of adapter type, speed, and physical directory. The
registry rejects duplicate stable identities. Registry adapters are trusted server
configuration: a cache adapter must own a namespace disjoint from authoritative
objects. No user-facing arbitrary-locator or adapter-registration endpoint is added.

`VaultAdapter` returns streams and metadata, with optional operation capabilities.
Unsupported operations fail explicitly. Presence distinguishes present, missing,
unreachable and unknown; probing never triggers a download. Remote adapters do not
claim range or presence support they cannot provide. Proxy playback continues to
implement progressive range reads over its existing in-flight buffer, while its
source adapter supplies the sequential stream. HTTP images retain existing complete
response buffering and image validation.

The filesystem adapter contains root-confined opens and immutable file exposure.
Audio consumers retain owned descriptors; local image reads and availability probes
also pass through the backend. Local publication uses a no-overwrite hard link from
staging, syncs directory entries, and removes the staging link. Manager journal
recovery handles interruptions around exposure and catalog attachment. Local staging
paths remain the scoped exception required by ffmpeg. Metadata journals and catalog
transactions remain coordinated by MediaManager, rather than by a remote adapter.

## Dependent-copy validity

`register_cache_copy` records an already complete immutable audio cache object in
`.media/cache-index`. It validates the registered vault role and exact authoritative
content version. This supplies the contract for later cache-placement policy; it
performs no automatic caching or multi-vault selection. The existing image cache
continues using its publication journal and source association.

Every `read_cache_copy` checks the current authoritative version before opening the
adapter and again after the open completes. Authoritative replacement or deletion
therefore rejects all old cached representations, even on an unreachable device.
No successful physical deletion is needed to establish invalidity. Catalog/database
errors fail closed. An availability projection alone is not a deletion: uncertain
physical observations cannot revoke content or trigger cache eviction.

`delete_authoritative_audio` conditionally deletes the exact current authoritative
descriptor, including ingested audio. Automatic proxy retention still uses the
existing provenance-restricted `remove_copy`. Both detach the authoritative catalog
reference through the recovery journal, making dependent copies invalid. Stale
requests cannot remove a newer publication. Catalog metadata remains intact.

`evict_cache_copy` only deletes its registered cache object, leaving authoritative
media intact. Repeated eviction is harmless. Evicted copy identities leave durable
`.deleted` records to prevent reuse during delayed operations. Replacement and
removal never cancel already-open readers; the validity checks govern new opens.

Startup and periodic maintenance retry physical cleanup of invalidated caches.
Unregistered/unreachable adapters leave durable records for later retry. Cleanup
rotates through at most 1,000 indexed records per call (directory enumeration itself
is not bounded). No distributed protocol or offline-serving lease is implemented:
future federation must revalidate with authority before serving after reconnect.

## Scope and tests

The implementation preserves the current single-writer-process assumption per media
root, existing publication recovery, retention thresholds, file protection, HTTP
permission checks, progressive buffering and image best-effort persistence.

`media/vault_tests.rs` uses fake adapters to cover multiple copies/representations,
replacement/deletion, offline cleanup, restart, stale requests, replacement during
an adapter open, explicit authoritative deletion, cache-only eviction, unsupported
operations and missing versus unreachable content. Filesystem contract tests cover
range reads, immutable publication, root confinement and idempotent removal.
Existing media tests cover real HTTP image retrieval/source association and proxy
streaming through the adapters, plus legacy publication/recovery regression tests.

Multi-vault selection remains #8. New placement/eviction policies, live configuration,
actual federation, YouTube integration, client cache migration and database response
caching are excluded. Generic orphan/old-generation garbage collection remains
separate from deletion of explicitly registered cache copies.
