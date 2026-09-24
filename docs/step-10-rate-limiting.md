# Step 10 HTTP rate-limit canary

The server now uses `simple_server::rate_limit::KeyedLimiter` and
`RateLimitLayer` for all previously tower-governor-backed HTTP routes.
Reviewed shared revision: `925ea25153a35e1fdb748add4880bc0e7d384ae6`.
Work started from `dev` at `3f2271ff`, in an isolated migration worktree.

## Preserved policy

- Global, content-read, stream, search, write, user-content-read and per-device
  analytics limits retain their independent/shared instances, route placement,
  exact integer-millisecond replenishment intervals and burst capacities.
- Login retains four buckets: IP burst/sustained and account burst/sustained.
  OIDC and password login still share the same IP buckets. Existing layer order
  preserves which budgets are charged when a later gate rejects the request.
- Identities come from the existing extensions: peer IP (port ignored), user ID,
  user/device pair, and SHA-256 of the exact account handle. Forwarded headers
  remain ignored. The account extractor still restores the buffered login body
  and enforces its existing 16 KiB maximum.
- Denials retain status 429, both Retry-After and X-RateLimit-After, and exact body
  `Too Many Requests! Wait for {seconds}s`. Seconds are rounded down, including
  zero for subsecond waits. Missing identity retains 500 and `Unable To Extract
  Key!`. The previously unused custom metrics/error handler is not newly mounted.
- Storage explicitly remains unbounded, matching existing policy. Login cleanup
  still runs every 600 seconds and removes fully replenished keys using `prune`.
  Other buckets do not gain new cleanup, caps, exemptions or proxy trust rules.
- Request and response bodies stream through the shared HTTP layer; it does not
  read login bodies itself. Application auth, authorization, CSRF and body limits
  remain in their original order.

Direct governor and tower_governor dependencies have been removed. The local
adapter only selects identity and formats the legacy response; budget accounting,
shared storage and HTTP admission execution belong to simple-server.

## Deliberately incomplete scope

Overall Step 10 remains **Partial**. MCP has three category counters with one
shared per-user window anchor, a strictly `elapsed > 60s` reset, accepted zero
limits, and an existing usage-inspection contract. The shared fixed-window budget
uses independent anchors and `elapsed >= window`; substituting it would change
behavior. A grouped-window API or an application-owned policy callback needs
separate review before migrating MCP.

Database-backed download/report quotas and outbound enrichment pacing remain
application-owned. This HTTP canary does not replace durable decisions with
in-memory counters or claim their adoption. It also does not claim completed
consumer Axum removal.

## Verification

Original focused rate-limit suite: **33 passed**. Two additional missing-identity
and real-HTTP response contract tests passed against the old implementation,
then all **35** passed with the shared implementation. Existing tests cover
IP reconnects, cross-IP account limits, device/user identity, and login body
restoration. Socket tests use isolated loopback listeners outside the sandbox.

Full `cargo test --offline`: **1,432 passed**, zero failed, 36 existing ignored
(including two doctests). This includes real production-router authentication,
permissions, catalog, MCP, reports, streaming, sync, WebSocket, lifecycle and
mixed-workload integration tests. `cargo clippy --all-targets --offline` passes
with warnings in untouched enrichment/background-task tests and an existing transitive
num-bigint-dig future-compatibility notice. No lint findings occur in the migration.
`cargo fmt --all --check` and `git diff --check` pass. Docker/release builds were
not rerun for this canary.
