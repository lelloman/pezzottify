# Shared HTTP core: embedding canary

Reviewed simple-server source: `64f31b41f36617f0269d14248f284c34c7dffe17`, recorded
in `simple-server.rev` for the existing checkout/CI script. The server enables
`web-compat`, which includes the opt-in shared `web` core.

The five embedding handlers (list, get, upsert, delete and search) and their two
route constructors now use simple-server's own Router, method routing, State,
Path, Query, Json and IntoResponse APIs. `src/server/embeddings.rs` has no Axum
import, trait bound, return type or response conversion. It retains the existing
shared `Extract<Session>` authentication.

Two explicit `web::compat::into_axum_router` conversions live in
`src/server/route_builder.rs`. They mount the migrated groups at their original
locations inside the legacy application. State injection, access/edit-catalog
permissions, content/write rate policies, CSRF and outer response/tracing layers
remain where they were. ApiError implements the shared response trait by calling
its existing buffered renderer, retaining status, JSON, request IDs and Retry-After.

Other routes, global middleware, streaming, multipart, SSE, WebSockets and test
server fixtures remain transitional. The conversions are temporary; this canary
does not claim full Axum removal from Pezzottify.

## Before/after verification

Baseline `dev`: `c27e1bdc`, shared `9e1c067` (documentation descendant of the
previous source pin). Existing auth **22**, permission **22**, and route-composition
**3** HTTP tests passed. Three new real HTTP tests were committed separately in
`f85af6a3` and passed against the original embedding handlers before migration:

- Upsert, encoded namespace, list/get query behavior, JSON metadata/vector data,
  nearest-neighbor search, delete/204, and missing-item error/request-ID contracts.
- Anonymous 401, non-admin write 403, and missing-CSRF rejection.
- Unsupported media type 415, JSON syntax 400/data 422, query 400, body limit 413,
  and application validation error codes.

After migration, all **50** targeted tests passed unchanged. The full Rust suite
passed **1,447 tests, with 36 existing ignores**. Production-target strict Clippy
passed; all-target Clippy passed with existing warnings in unchanged enrichment
and background-task tests, plus the existing num-bigint-dig compatibility notice.
Changed-file formatting and diff checks passed. Shared validation: **222 tests/
doctests**, strict all-target Clippy and independent web/extract feature checks.
The central migration trackers record the integration commit and cleanup. Builds use isolated targets, two jobs, disabled
dev debug information, offline/locked dependencies and the `fast` fixture feature.
External OIDC providers, Docker, browser and Android tests are outside this check.
