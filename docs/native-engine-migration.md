# Native engine migration

The production main function uses `#[simple_server::main]`. Its application
futures, HTTP/metrics listeners, WebSocket transport, HTTP clients, tasks, timers,
file I/O, process waiting and logging use simple-server's shared engine. The
`execution` and `web` modules contain application import/route adapters, not a
second runtime. The server's existing graceful drain and admission policies are
preserved. Ordinary database worker threads explicitly enter the engine context
when they need asynchronous work.

The `build_search_index` helper adopts shared logging and links the engine.
`cli-auth` and `query_search_index` remain synchronous helpers and do not link an
unused native runtime; their database/search behavior is unchanged.

SQLite (including hooks/FTS and the bounded database executor), search, JWT/JWKS
verification and refresh, Web Push crypto, RSA/Argon2 and ZIP remain local.
OpenID Connect uses the original SDK with a shared HTTP adapter. Host Tokio is
limited to synchronization/macros through simple-server; it has no production
executor, timers or socket drivers. Independent test clients retain Tokio and
Reqwest as development dependencies.

## Building and distributing

`pezzottify-server/simple-server.rev` pins both the bindings and engine source.
Run `bash pezzottify-server/scripts/checkout-engine-source` for a fresh sibling
checkout; an existing checkout is never rewritten. Use Rust 1.96 or newer.
From the backend directory:

```sh
bash scripts/build                  # all release binaries, with lib/ installed
bash scripts/test                   # default tests
bash scripts/test --features fast   # CI test configuration
bash scripts/run -- --config ./config.toml
```

The first call builds the pinned engine once unless `SIMPLE_SERVER_ENGINE_DIR`
points to a supplied artifact directory. Supplied artifacts require the library,
`SOURCE_REVISION` and `SHA256SUMS`; checksum and revision mismatches fail before
building the application. No published artifact is assumed or uploaded.

Distribute the four release binaries together with
`target/release/lib/libsimple_server_engine.so.1`. Each binary uses `$ORIGIN/lib`.
The `.so` must match the architecture, system ABI and pinned bindings; this is
not a mechanism for arbitrary hot-swapping incompatible engine versions.

`build-docker.sh` and `e2e/run.sh` prepare ignored `.engine-build` inputs before
Docker runs. For a direct Docker build, first run
`bash pezzottify-server/scripts/prepare-docker`. The prepared source is a clean
Git export of the pin and the prepared engine is checksum verified. The Docker
consumer build links that artifact; it does not compile the engine again.
The images use Debian Trixie; supply a glibc-compatible Linux engine. The seed
image uses the same Debian release and needs no engine for its synchronous cli-auth.

## Verification status

Implemented on an isolated branch based on dev `5fab6e4f`, preserving the
intervening frontend commit. Shared implementation/source pin:
`f21dbfeacf87c1108f7c3d4d1578b2017b201272`.

- Baseline and migrated default suites: **1,561 passed, 38 existing ignores**.
- Migrated `fast` suite: **1,561 passed, 38 existing ignores**.
- Formatting, production strict Clippy and database boundary checks pass.
- The broader all-target strict Clippy check also fails on the original checkout:
  items after test modules, an unused read count and test helper warnings. It is
  not claimed green. Production CI retains its existing Clippy scope.
- The shared engine check passes 233 tests (three existing ignores), standalone
  consumer/C ABI fixtures and dependency checks. Additional WebSocket cancellation,
  close-frame, process-output and panic-diagnostic contracts pass. Shared
  all-feature/all-target strict Clippy and rustdoc pass.
- Logging compares legacy/native behavior in 60 configurations. Real production
  process tests cover SIGINT/SIGTERM/admin reboot, listeners, WebSocket drain and
  startup failure. HTTP tests cover auth/permissions, rate limits, uploads,
  ranges/streaming, SPA fallback, jobs and persistence.
- Docker image build and network-disabled server/CLI loader smoke pass. The
  complete browser/Android Docker E2E suite has not been run for this migration.
- Artifact revision/checksum rejection checks pass, including relative artifact
  directories. The release wrapper builds all four binaries; graph/link checks
  pass. A relocated bundle launches without LD_LIBRARY_PATH through `$ORIGIN/lib`.
- Normal/build dependencies: **361 → 328 package/version entries**, **340 → 307
  package names**. No host Axum/Hyper/Reqwest/Rustls/tracing-subscriber; host Tokio
  retains only synchronization/macros.

Implementation commit: `0b666437`; branch integration remains pending.
No push, publication or deployment is claimed. The new source revision must be made
available remotely separately before a fresh remote CI checkout can fetch it.

## Build measurements

Three alternating paired fresh-target builds per profile, eight jobs, Rust 1.96.0,
offline cached dependencies; engine prebuild excluded:

| Profile | Clean before | Clean after | Reduction | Main touch before | Main touch after |
|---|---:|---:|---:|---:|---:|
| dev | 66.214s | 56.872s | 14.11% | 1.736s | 1.592s |
| release | 114.179s | 105.151s | 7.91% | 5.953s | 4.836s |

Touch measurements change only `src/main.rs`'s timestamp, not library code.
Release samples varied (before 110.9–114.4s; after 97.4–108.9s); the table reports
medians, not the best pair. No runtime speedup is claimed.
[Raw measurements](measurements/pezzottify-engine-build-2026-10-07.json).
