# Step 03c: HTTP tracing

Production `route_builder::build_app` already installs `log_requests` outside
session/auth/CSRF middleware. It now calls shared `web::tracing::trace_with_observer` when
`RequestsLoggingLevel` is Path, Headers or Body. None bypasses tracing completely
and retains existing metrics/bandwidth accounting. The separate metrics listener
and offline CLI tools did not have HTTP request logging and remain unchanged.
The reviewed library pin is `ca98a4159e1cb0dd7b9db2faa9a076d198b7973b` in
`simple-server.rev`; CI/build checkout scripts consume that pin.

## Behavior and telemetry schema

The old `>>> METHOD path` and `<<< status (milliseconds)` lifecycle lines are
replaced by the shared safe `http.request` span and `http.response_headers` /
`http.finished` events. Request paths become matched route templates (unmatched
routes use the shared placeholder); query strings, credentials and opaque
incident IDs are not added to spans. Header timing measures response creation;
terminal timing covers body polling/completion/error/drop or upgrade handoff,
not client acknowledgement or WebSocket session lifetime. A small application observer preserves the existing INFO response-header
event severity for every status, including immediate visibility for long-lived
streams; terminal events delegate to the shared default observer.
User-specified LOG_LEVEL filters are preserved and can select the shared
`simple_server::http_tracing` event target.

Opt-in allowlisted header and redacted JSON-body diagnostics remain local,
including authentication/report exclusions and the existing small-body buffering
policy. The shared wrapper encloses that processing, including body-read failure
responses, so it observes the final returned body. The existing Prometheus header
latency/counts, endpoint bandwidth counters, authenticated usage accounting,
response status/headers/body, authentication boundaries and per-error incident
ID policy are retained. Correlation is intentionally not enabled by this change.

## Verification

The isolated migration worktree was created from active `dev` at
`b82e17d410fd35b048f54993e686bd2651fa8d26`; the original checkout was clean.

Baseline library tests: 1103 passed, two existing live-model/API tests ignored.
Final library tests: the same 1103 passed/two ignored, including the new
configured-mode/safe-route/unchanged-response contract replacing the legacy
path-only test. The loopback production-router integration test passes for all
four logging modes, preserving empty auth rejection responses and authenticated
404 behavior while checking safe route fields, INFO header events and absence
of raw path/query/credential values or duplicate legacy lifecycle lines.
Three existing route contract tests and all four production-binary lifecycle
tests pass (SIGINT, SIGTERM, admin reboot, occupied listener), including active
WebSocket/listener drain. Changed-file rustfmt and `git diff --check` pass.
Strict Clippy remains blocked by the unchanged `items_after_test_module` finding
in `src/enrichment_store/works.rs:247`, also documented during Step 03a.
Clippy for the library and all test targets passes with that single existing
lint explicitly exempted and all other warnings denied. All 11 existing HTTP
streaming/range tests pass. Total executed final checks: 1122 passed, two
existing ignored tests. Other integration suites, browser/Android builds,
release/container builds and external services were not rerun. Nothing was
pushed or deployed.

The migration is committed in its dedicated worktree, then the original `dev`
branch is rebased onto it with ancestry/tree verification before removal of the
temporary worktree and branch. Exact final commit and integration evidence are
recorded in the central simple-server migration trackers.

## Backend-independent observer canary — 2026-09-26

The production middleware now implements `web::tracing::Observer` and receives
`ResponseInfo` instead of an Axum response. Completion delegates to the owned
`TracingObserver`; response headers retain Pezzottify's INFO severity and
`simple_server::http_tracing` target. The shared implementation uses the same
streaming lifecycle engine. Logging modes, metrics, diagnostics and routing
placement are unchanged. `web-compat` remains required for other protocols.

The strengthened real-HTTP regression checks exactly one header and completion
event per rejected request when logging is enabled, no such events in None mode,
the INFO severity and stable target, plus existing safe-route, secret exclusion,
response-body and authenticated-404 contracts. It passes against the original
library pin before the production migration.

Baseline and final `cargo test --locked --features fast -j 2` each pass
**1,451 tests**, with **36 existing ignores**. The strengthened real-HTTP
contract also passed separately before migration. Formatting, database execution
boundary checks, strict production `cargo clippy --locked -- -D warnings` and
default-feature `cargo build --locked` pass. Existing test unused-import warnings
and the num-bigint-dig future-compatibility notice remain. Docker, Android,
browser and release builds were not rerun for this observer-type migration.

Work was isolated from active `dev` at `9115a8a4`. The exact shared library pin
is recorded above and in `simple-server.rev`; CI and build checkout scripts
consume that file. Integration and cleanup evidence are recorded in the central
migration trackers. Nothing pushed or deployed.
