# Step 03a: logging setup

Both existing subscriber entrypoints now call the shared logging adapter:
`pezzottify-server` in `main.rs::run`, and the offline `build_search_index`
binary. The source pin is `71755b5e15ada9b22484559146ebaf4d82c91255`, recorded
in `simple-server.rev` for the existing checkout/build scripts. Other CLI
binaries installed no subscriber and remain unchanged.

The server keeps its lossy `LOG_LEVEL` parser with INFO default; the indexer
keeps its lossy `RUST_LOG` parser with ERROR default and its existing policy of
ignoring initialization errors. Both retain text on stdout, targets, fields,
span context, historical NO_COLOR behavior, and log-facade forwarding via an
explicit tracing-log bridge. Normalized filters are passed to the shared API
(empty directive sets map to `off`). No correlation or HTTP tracing was added.

## Verification

The migration worktree was branched from active `dev` at `fb7270bf`.
Untouched baseline library tests passed 1103 tests with two existing live API /
model tests ignored; sandbox-denied local socket tests were rerun unrestricted.
The final complete Rust suite passed 1398 tests with 36 existing ignored
tests/examples; this includes the same 1103 library tests. All four production-server
lifecycle tests pass (SIGINT, SIGTERM, admin reboot, and occupied listener),
including active listener/WebSocket drain with isolated databases. The logging comparison
uses the actual production adapter and both production parsing policies in
fresh processes over 60 filter/color/entrypoint combinations, checking missing,
empty, whitespace and invalid filters, levels, target/span filters, structured
fields, stdout/stderr, ANSI, and legacy log records. The indexer also reaches
its expected missing-catalog startup error after invoking shared logging.

Changed-file rustfmt and diff checks pass. Strict all-target Clippy is blocked
by the unchanged `items_after_test_module` finding in
`src/enrichment_store/works.rs:247`. No browser/Android/release image,
deployment, or push checks were performed.
