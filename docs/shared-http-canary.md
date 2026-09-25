# Shared HTTP core: complete routing adoption

Reviewed simple-server source: `d3b559ad7a3597a29c4a5be77c82531f2e1e8c25`, recorded in
`simple-server.rev`. The server enables `web-compat`, which includes shared `web`.

All route groups now use shared Router/method routing, ordinary extraction and
response contracts. Application state implements `FromState`; byte-range parsing
uses shared `FromRequestParts`. ApiError has one shared response implementation.
The two legacy embedding-router conversions have been removed.

Permissions, CSRF, report admission, rate limits, cache policy and logging compose
with shared middleware and Tower layers in their existing order. Static files
use `fallback_service`. Main HTTP and test servers use shared serving with direct
TCP peer metadata; metrics uses shared serving without connection metadata.
Rate-limit identity continues to ignore forwarded headers. Audio ranges and body
streams preserve their existing behavior through the shared Body type.

Explicit remaining backend contracts: multipart fields/errors, SSE event streams,
MCP/sync WebSocket sockets/messages, the tracing observer, independent mock HTTP
servers and the error-renderer differential test. Protocol adapters live under
`web::compat`; Step 11 completion does not claim complete Axum removal.

## Routing completion verification

Baseline `dev`: `bf825a5d`; existing full suite: **1,447 passed, 36 ignored**.
Two added HTTP tests passed before production edits (commit `370f25c4`): multipart
auth/parser/field errors, HEAD response stripping, 405/Allow and unmatched 404.

After migration: **1,449 passed, 36 existing ignored**, including all protocol,
authentication, permissions, reports, body limits, rate limits, tracing and route
suites. Strict production Clippy passes with `fast,slowdown`. Formatting and diff
checks pass. Shared library: **231 tests/doctests**, strict all-target Clippy,
**14 minimal-web tests** and **1 extract-only test**. The nine new shared tests
cover layer order/rejection, metadata, readiness, fallback services, standard body
adapters, cancellation, trailers/errors and direct-peer serving/shutdown.

Builds used offline/locked dependencies, two jobs, isolated targets and disabled
dev debug information. Existing num-bigint-dig future-compatibility notice remains.
Docker, browser, Android and external OIDC-provider tests were not run. The central
trackers record base-branch integration and removal of the owned worktrees,
branches and temporary build/log files. No push or deployment.

## Historical embedding canary

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
