# Step 06 background tasks canary

Pezzottify adopts the optional `simple-server` task, scheduling and execution-policy
primitives in its production server. Its existing scheduler still owns job
registration, bounded command/hook channels, durable history and schedule state,
metrics, audit logging, and domain cancellation tokens. No database schema or
admin HTTP response schema changes are required.

## Production adoption

- **06a:** HTTP/WebSocket/MCP upgrade reservations and tracked request/maintenance
  work use `WorkTracker`. Upgrade ownership is acquired before the callback and
  is rejected after admission closes. Each background execution has a bounded
  `TaskSet` owner. Shutdown joins wrappers through completion, including their
  blocking jobs and database finalization. Interrupting the drain retains the
  owners for a subsequent drain.
- **06b:** persisted interval recurrence uses `Schedule::FixedDelay` and global /
  resource-class execution limits use `ExecutionCapacity`. One execution per
  registered job includes queued jobs; default limits stay global 4, general 4,
  lightweight 4, I/O 2, CPU 1. Capacity acquisition remains class before global.
  Queued cancellation/timeout drops partially acquired permits. Runtime expiry
  never releases a still-executing blocking job's permits.
- **06c:** queue/runtime deadlines use `ExecutionBudget`; persisted circuit
  transitions use `CircuitBreaker` and snapshots; pause admission uses `PauseState`.
  Existing JSON formats, circuit failure classification, response codes and
  persistence failure policy are preserved. A lower configured threshold can
  load an older closed failure count without panicking.

## Behavior retained deliberately

The application keeps its wall-clock persisted schedules, inclusive whole-second
positive jitter, immediate/deferred first-run choice, and manual/hook runs resetting
interval dates. The shared runnable scheduler's different trigger/pause semantics
are not substituted. Pausing blocks new admission; accepted queued jobs retain
Pezzottify's existing behavior. Pause-and-cancel and explicit cancellation respect
the job's declared cancellation support. Parent application cancellation tokens
retain their existing propagation; waiting for completion always joins the work.

Queue budgets cover class/global semaphore waits. Runtime budgets start before
Tokio blocking dispatch, as previously, and timeout history stays `Job timed out`
after the blocking job returns. Application errors and panics remain history
entries; they do not shut down unrelated jobs. Queue expiry and cancellation do
not count as circuit failures. There is no new automatic retry policy: jobs retain
their application-specific durable retries. Declared but previously unimplemented
cron variants remain unchanged; this canary does not activate cron execution.

History-start failure still prevents execution (the established trigger endpoint
continues to acknowledge the request). Pause persistence must succeed before live
state changes. Startup marks abandoned history failed and restores existing
schedule/control records; no exactly-once or durable shared-engine claim is made.

Completion now wakes the scheduler immediately, so it updates recurrence without
waiting for its old idle poll. A previous execution is collected before replacing
its owner. Shutdown wins over simultaneously ready commands, and interrupted
draining keeps execution ownership in the scheduler.

## End-to-end evidence

`tests/e2e_background_tasks.rs` exercises real loopback HTTP, the production
application scheduler, controlled blocking jobs, and isolated SQLite databases.
Its 23 cases cover payload/history round trips, concurrent trigger deduplication,
cancellation and retriggering, global/class limits, queue/runtime expiry, retained
blocking capacity, errors/panics, composed pause scopes, cancellation support,
shutdown draining, circuit opening/recovery, restart restoration, stale history,
hooks, interval recurrence, deferred jitter/manual reset, failed history and pause
writes, queued cancellation, panic/timeout circuit outcomes, combined schedules,
overdue schedules without backlog replay, authentication, and threshold changes.
Restart cases reopen the SQLite connection rather than reusing in-memory policy
objects. SQLite triggers inject actual storage write failures.

`tests/e2e_lifecycle_tests.rs` runs the actual binary against temporary databases
and ephemeral ports: SIGTERM, SIGINT, admin reboot, occupied listeners, WebSocket /
MCP drain, plus persisted pause restoration across a full process restart and
subsequent HTTP trigger rejection/resumption. Focused ownership tests also cover
late admission rejection, guard release on panic, reserved upgrade draining, outer
shutdown deadlines, and interrupted/resumed scheduler draining.

Baseline before changes: 127 background-job tests passed, two existing tests
ignored; 16 admin-job E2E and four production-process lifecycle cases passed.
Final verification and integrated revisions are recorded in the central
`simple-server/docs/migration-status.md` and HTML table.


## Verified commands

Reviewed shared library: `4a6353f55b23dff173ec1968915c6e10312d5795`
(`simple-server.rev` also drives CI and the checkout script).

From `pezzottify-server`:

```sh
cargo test --features fast
cargo test --features fast --lib background_jobs
cargo test --features fast --test e2e_background_tasks --test e2e_lifecycle_tests --test e2e_admin_jobs_tests
cargo clippy -- -D warnings
cargo fmt --check
bash scripts/check-db-boundaries.sh
cargo audit
```

The full suite passed **1,429 tests**, with **36 existing ignores**. After the
final drain-resume guard and obsolete-import cleanup, all **128 background-job
tests** (two existing ignores) and **44 focused HTTP/process E2E cases** passed
again, along with strict CI Clippy. The new process restart test selects local
`device_pruning` maintenance explicitly, avoiding random outbound-network jobs.
The audit retains six existing allowed warnings; no audit policy was changed.
The release container and non-Android Docker API/browser suite are also checked
using an isolated Compose project, fixture volumes/network, and image tags.
This is local verification, not deployment or live-production validation.
