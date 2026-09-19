# Step 02: application lifecycle

Pezzottify uses `simple-server`'s `lifecycle` feature from revision
`c5359079ff4fad0b4b0359e8c88880dbbbc4eeb5`. The sibling checkout, revision file,
and Docker named context follow the existing Step 01 arrangement. The Docker
context now also includes the library's examples. The revision must be available
to the checkout script's remote before remote CI can fetch it; this migration
itself does not push or deploy either repository.

## Runtime ownership

The application retains Tokio, CLI/configuration, logging, database setup,
router composition, and process exit policy. `main` installs shared SIGINT/SIGTERM
handling. `prepare_server` binds both sockets and registers HTTP services before
readiness; listener failures are returned as startup errors rather than panics.
Peer socket addresses remain available to authentication rate limiting.

One coordinator owns the API listener, metrics listener, scheduler, event pruning,
storage metrics, passive WAL checkpoints, playback maintenance, and media recovery.
SIGINT, SIGTERM, an admin reboot request, or an unexpected service exit initiates
shutdown. Both HTTP listeners drain concurrently; the scheduler receives its
existing cancellation token and remains polled while joining running jobs.
The old per-job 30-second timeouts have been removed.

The single **30-second budget** covers all service draining and subsequent
application-task cleanup. Sync and MCP WebSockets hold tracker tokens allocated
before upgrade, observe shutdown, and close their connections. Sync sessions
also join their outgoing task and unregister playback/connection state.
The tracker also covers login-limiter/report maintenance, streaming-search
producers, and request-spawned ingestion processing. Its wait starts after HTTP
has drained, so requests and pending upgrades cannot escape the cleanup wait.
Maintenance stops taking new periodic work; already-running blocking work is
awaited. Download-manager initialization is awaited before serving. The disabled
stale-batch loop, whose callback did nothing, has been removed.

The admin reboot endpoint still returns HTTP 202 and its existing body, but now
requests coordinated shutdown instead of exiting after an arbitrary delay.
Successful shutdown exits normally; startup/lifecycle failures and deadline
expiry exit nonzero. On failure, explicit process exit prevents Tokio teardown
from waiting indefinitely for a timed-out blocking job. Development and E2E
Compose services allow 35 seconds before forced termination, leaving room for
the application budget. External deployment managers should allow the same;
the homelab deployment configuration is outside this repository.

## Boundaries

- The existing resumable search-index build uses its own OS thread. It is not
  joined by this coordinator; its persistence/recovery behavior is unchanged.
- Blocking operations cannot be forcibly cancelled. The deadline bounds the
  coordinator's asynchronous wait, and failure exits the process; it is not a
  hard real-time guarantee for synchronous code blocking an executor thread.
- The router-only `make_app` helper retains its existing runtime-owned maintenance
  behavior for integration tests/embedders. Production uses `prepare_server` and
  the coordinator. Merely constructing a router does not install signals or
  provide the process lifecycle contract.
- No new final database checkpoint, job restart/supervision policy, runtime, or
  deployment policy is introduced.

## Verification

Real-binary integration tests in `tests/e2e_lifecycle_tests.rs` cover SIGTERM,
SIGINT, reboot, active sync/MCP WebSockets, and both occupied-listener cases.
Tracker tests cover upgrades reserved before callbacks start and cleanup deadline
expiry. Existing scheduler tests cover cancellation and job completion.

Run from `pezzottify-server`:

```sh
cargo test --locked --features fast
cargo check --locked --all-targets --features fast
cargo clippy --locked -- -D warnings
bash scripts/check-db-boundaries.sh
cargo fmt --check
cargo audit
```

Run Docker E2E from the repository root:

```sh
COMPOSE_PROJECT_NAME=pezzottify-step02-e2e ./e2e/run.sh tests -m 'not android'
```

Baseline `25a531f5` passed 1,390 Rust tests (34 ignored tests, plus two ignored
doctests). The migration adds four production-binary tests and two tracker tests; the full
migrated suite passes **1,396 tests**, with the same 36 total ignores.
The final WebSocket/lifecycle subset passes all 14 tests, including actual close
frames for sync and MCP sockets. Locked all-target checking, strict Clippy,
formatting, database-boundary checks, and both Compose configurations pass.
`cargo audit` succeeds with six existing allowed warnings; no unrelated dependency
versions were changed.

The final Docker release/frontend build and all **45 non-Android Docker E2E
tests** pass (two Android cases deselected). The E2E runner reports only its
existing read-only pytest-cache warnings. No deployment or push was performed.
