# Steps 08/09: shared authentication and route authorization

Pezzottify's initial auth migration adopted `simple-server::auth` from reviewed
revision `0a629da7b5eb5aeeb0ed64aac2c5f96cd4d9717b`. The original `auth`
feature is enabled in `pezzottify-server/Cargo.toml`; `simple-server.rev`
records the active reviewed revision for the existing CI and checkout script.

`session::validate_session_token` now evaluates an `AsyncAccess` verifier. This
path serves HTTP session extraction and long-lived transport revalidation. The
application still validates OIDC first and then looks up the legacy database
token. It still provisions OIDC users, loads a fresh permission snapshot, tracks
devices, and maps invalid tokens to 401 and executor errors through the existing
response policy. The shared flow does not cache a session or change revocation.

Every named HTTP route policy in `server::authorization` now evaluates `Access`.
The Axum `Session` extractor authenticates first; the access check uses its
permission snapshot and returns the established 403 on denial. Public routes,
handler-specific resource policy, 404 concealment, mutation-sensitive checks,
and CSRF stay in the application.

The application's Authorization parser remains in place. It enforces token68,
allows multiple spaces after a case-insensitive Bearer scheme, and optionally
accepts legacy raw tokens. It rejects duplicate or malformed headers without
falling back to a valid cookie. `HeaderCredential` alone does not implement
that exact compatibility grammar. Header precedence, cookie names, OIDC-to-legacy
fallback, status codes, and response bodies are preserved.

## Verification

The migration adds a real HTTP regression for duplicate Authorization values
with a valid session cookie. The existing auth suite covers Bearer and raw
tokens, strict mode, malformed-header fallback, login, logout, CSRF, and cookie
policy. Permission tests cover unauthenticated, regular, and admin requests;
MCP tests cover revocation and permission changes on an open connection.

The isolated baseline used Pezzottify `5ea9ed6bb8206943b86d883738ae78fae39046ee`
with its original shared pin `4a6353f55b23dff173ec1968915c6e10312d5795`.
`cargo test --offline --locked --features fast --lib session::tests` passed 32.
The three existing HTTP suites passed 19 auth, 5 MCP, and 22 permission tests.

The migrated tree passed the same 32 focused unit tests and 20 auth, 5 MCP, and
22 permission HTTP tests, including the new duplicate-header regression. Both
worktrees used private Cargo targets, two build jobs, and debug info disabled;
HTTP tests used temporary fixtures and local loopback ports. `cargo fmt --check`
and the repository's strict `cargo clippy --offline --locked --features fast --
-D warnings` passed. An expanded `--all-targets` Clippy run found an existing
`items_after_test_module` warning in unchanged `src/enrichment_store/works.rs`;
its source is identical in the baseline checkout. Initial sandbox HTTP attempts
failed at fixture port bind before any request ran; the same tests passed with
loopback access. The focused checks cover the changed authentication and route
authorization paths; unrelated suites, browser/Android, and container checks
were not rerun.


## Cookie/header selection follow-up — 2026-09-25

The active shared revision is `d7e8d133bfc67b62b93d2679fe27610de0dfda99`.
Production session extraction now invokes `AuthLayer::credentials` through its
lazy `authenticate` API. This preserves existing extraction points, public
routes, fresh permission snapshots and database side effects. Required and
optional Session adapters remain thin Axum bridges until the shared handler API
exists; the optional adapter continues treating invalid credentials as anonymous,
while database failures retain existing error responses.

The shared selector gives Authorization precedence and rejects duplicate/invalid
text without cookie fallback. The application retains its token68 grammar,
case-insensitive Bearer with extra spaces and opt-in legacy raw credentials.
Verification still tries OIDC then the existing database session resolver.
Verified source metadata is installed as `Identity<Session>`; it is cleared and
revalidated on each extraction rather than reused as a cache.

`auth-cookies` supplies decoded-cookie compatibility: percent decoding, ignored
invalid header text/unparseable pairs, last duplicate wins and empty values
preserved. This matches the previous CookieJar behavior. Session and CSRF cookie
reads use the shared parser; creation/expiration use framework-independent
shared cookie value types. The direct `axum-extra` dependency and CookieJar
extractor are removed. Existing cookie attributes, login/logout Set-Cookie
behavior, CSRF header-presence exemption and route exemptions are preserved.
No new CSRF policy or global authentication middleware is installed.

Baseline: clean dev `4a7e9ee190820352ba278fa663da1f11666432ef`, unchanged consumer
code against shared `9ff4476`. Session-focused tests: 32 passed. Auth/MCP/permission
HTTP tests: 22/5/22 passed, including two new tests run before migration for
percent-encoded cookies, duplicate ordering, whitespace and optional anonymous
sessions. Builds use private targets, the `fast` fixture feature, two build jobs
and disabled dev debug info.

Final full Rust suite: **1443 passed, 36 existing ignored**, with no failures.
This includes auth 22, MCP 5 and permission 22 HTTP tests plus cookie attributes,
CSRF, executor error mapping, streaming, lifecycle and WebSocket coverage.
Production-target strict Clippy passed. All-target Clippy passed with existing
warnings in unchanged enrichment/background-task tests, plus the existing
num-bigint-dig future-compatibility notice. Formatting and diff checks passed.
Source/manifests/lockfile contain no remaining axum-extra or CookieJar usage.
Shared extension: 208 tests/doctests and all-target strict Clippy passed; the base
auth-only dependency graph still excludes Axum/Tokio. Docker/browser/Android and
external OIDC-provider deployments were not exercised; this is local integration.
