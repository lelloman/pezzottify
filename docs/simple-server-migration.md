# Axum centralization: Pezzottify

Pezzottify is the first service adopting `simple-server` for Axum dependency
centralization. This step keeps the existing routers, state, middleware,
authentication, database setup, and application lifecycle.

## Dependency boundary

- Replace the direct `axum` dependency with the sibling `simple-server` crate.
- Import Axum through `simple_server::axum` in production code and tests.
- Enable `ws` and `multipart` on `simple-server`.
- Resolve Axum **0.8.9**, replacing the previously locked **0.8.1**.
- Keep `axum-extra` 0.10.0 and `tower_governor` 0.8.0; both resolve against the
  same Axum version, as does the transitive Tonic dependency.

The targeted lockfile update also advances axum-core and the WebSocket server
dependencies. Cargo advances Tokio from 1.43.0 to 1.50.0 for the resolved graph.
The existing WebSocket test client retains tokio-tungstenite 0.26.2.

The Axum re-export is intentionally transitional. Later modules will expose
`simple-server` interfaces incrementally; this change does not adopt a new
runtime or lifecycle coordinator.

## Reproducible checkout

`simple-server.rev` records the reviewed library revision. From the repository
root, `bash scripts/checkout-simple-server.sh` creates or verifies the sibling
checkout without overwriting existing local work. All Rust CI jobs and Docker
E2E setup use this revision. Docker receives the sibling through the existing
`simple-server-source` named build context.

A path dependency alone does not pin source content. After testing a library
change, update `simple-server.rev` explicitly. Local coordinated development
may use an edited sibling without running the strict checkout verifier.

## Verification

From `pezzottify-server`:

```sh
cargo tree --locked -i axum
cargo check --locked --all-targets --features fast
bash scripts/check-db-boundaries.sh
cargo clippy --locked -- -D warnings
cargo test --locked --features fast
cargo build --locked --release
```

The Rust suite passed 1,374 tests with 36 existing ignored tests. This includes
HTTP, authentication, permissions, route contracts, streaming, WebSockets, and
mixed-workload tests. The production release build and Clippy passed. A redundant
borrow in the existing radio error response was removed to satisfy Clippy.

The Docker suite is run from the repository root in an isolated Compose project:

```sh
COMPOSE_PROJECT_NAME=simple-server-pezzottify-e2e ./e2e/run.sh tests -m 'not android'
```

All 45 selected Docker E2E tests passed; the two Android tests were deselected.
This includes OIDC/password login, administration, catalog browsing, download
requests, playlists, recommendations, browser synchronization, and mixed load.

## Existing check failures outside this migration

- Repository-wide `cargo fmt --check` reports existing formatting differences
  in unchanged catalog, queue, enrichment, media-module, and route-test files.
  Modified Rust files pass individual rustfmt checks.
- `cargo audit` reports RUSTSEC-2026-0285 for the pre-existing rustls 0.23.35
  dependency; its stated remediation is rustls >=0.23.45. Additional allowed
  warnings concern existing dependencies. This migration does not suppress the
  advisory or upgrade unrelated TLS dependencies.
