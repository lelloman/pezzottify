# Owned SSE search canary

Production `/v1/content/search/stream` uses `simple_server::web::sse`
(`Event`, `KeepAlive`, `Sse`) and shared `IntoResponse` directly. Reviewed
library: `96c542c2935606cbae48573e6d5ee634ed24970c`, recorded in
`simple-server.rev` for existing CI/build checkout scripts.

Only the SSE transport types and response conversion changed. Cookie/session
auth, search phases and section JSON, bounded channel capacity 32, tracked
producer ownership, one final Done marker and 15-second keepalive are unchanged.
Search does not implement event IDs/replay; Last-Event-ID continues to be ignored.
Multipart still requires `web-compat`; independent test mocks/parser oracles
still use backend types.

## Verification

Baseline `cargo test --locked --features fast -j 2` at the original library pin
`ca98a4159e1cb0dd7b9db2faa9a076d198b7973b`: **1,451 passed, 36 existing ignores**.
Eight strengthened real-HTTP search SSE tests passed before migration: no 404
skips, strict auth rejection, exact content/cache headers, JSON framing including
Unicode queries, unchanged reconnect-header behavior and exactly one final Done.
Final full suite: **1,452 passed, the same 36 ignores**, including those checks.
Formatting, DB-boundary checks, strict production Clippy and default build pass.
Existing test unused-import warnings and num-bigint-dig future-compatibility
notice remain. Docker, Android, browser and release builds were not rerun.

Work was isolated from `dev` at `ff7af524` in `migration/owned-sse`. Integration
uses the development-branch rebase and worktree cleanup workflow. Central
HTML/Markdown trackers record adoption and remaining boundaries. No push/deploy.
