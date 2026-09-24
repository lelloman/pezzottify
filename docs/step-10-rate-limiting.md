# Step 10 rate-limit adoption

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

## Completion of the remaining service scopes

The later migration completes Pezzottify's Step 10 scopes against shared
revision `799e94b47ddab1c04ce908c2afa6ea1c28e6e5d4`, recorded in
`simple-server.rev`. Production MCP tool and resource calls now use shared
grouped-window counters under the existing per-user
mutex and map. The three categories keep one anchor, a reset strictly after 60
seconds, accepted zero limits, legacy whole-second retry rounding with a one-second
minimum, non-resetting usage inspection, and the existing five-minute cleanup
rule.

Download request status still reads the calendar-day count and active queue size
from SQLite. Shared ordered limit checks make the signed comparisons used by
both request paths; admin exemption and the existing enqueue/count sequence stay
in the service. That sequence does not reserve admission atomically and retains
its pre-existing concurrency behavior. The system-capacity status type also uses
the shared comparison, although it has no production caller.

Report admission still uses an immediate SQLite transaction with idempotent
replay before quota evaluation. The shared rolling-window evaluator computes the
strict hour, day, and daily attachment-byte cutoffs from the one timestamp
sampled by the service; SQLite remains authoritative for counts. Shared ordered
checks apply user quota before global capacity, with saturating projected byte
and audit-reservation arithmetic. Rejections do not insert a partial report.
Report settings reductions use the same projected-capacity policy.

The active MusicBrainz and Last.fm enrichment clients now pace outbound calls
with a shared one-unit replenishing budget, retaining their 1100 ms and 200 ms
intervals. Each client holds its local mutex across waiting, charges immediately
before the request, and counts failed requests. The first request remains
immediate. The budgets are process-local, as the earlier pacing was.

OIDC provider metadata/JWKS refresh retains its separate auth cache policy:
15-minute freshness and a 30-second retry interval after discovery attempts.
It is a key-refresh recovery control rather than an enrichment request quota,
and remains owned by the OIDC client.

This migration does not claim completed consumer Axum removal.

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

For the follow-up migration, baseline focused tests before source changes passed:
MCP **5**, report repository **11**, and download manager **189**. Three MCP
legacy-oracle tests also passed before replacing its local counters. Against
the frozen shared revision, focused tests passed: MCP **9**, report repository
**13**, and download manager **190**. The full `cargo test --offline` server
suite passed **1,440** tests with zero failures and 36 existing ignored cases,
including the production MCP, download and report HTTP contracts. The outbound
pacer test was then made
deterministic; both final pacing tests passed. `cargo clippy --all-targets
--offline` passed with warnings only in unchanged enrichment and background-task
test files, plus the existing transitive `num-bigint-dig` future-compatibility
notice. `cargo fmt --all --check` and `git diff --check` passed. Docker/release
builds were not rerun for this follow-up.
