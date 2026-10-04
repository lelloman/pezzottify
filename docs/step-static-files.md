# Shared static files and backend dependency cleanup

The production frontend route uses `simple_server::web::static_files::StaticDir`
with directory indexes enabled and an explicit `index.html` fallback. This keeps
Pezzottify's prior behavior: unknown SPA routes and missing asset paths serve the
frontend, GET/HEAD and range/conditional requests retain their file semantics,
and unsupported methods return 405. API route registration and middleware order
are unchanged. An absent frontend configuration still uses the existing home
handler.

The migration targets public `lelloman-simple-server =0.1.3` with `static-files`.
The reviewed library implementation is `dbc7f68`; the full library checks pass
625 test executions, strict Clippy and rustdoc, and its publication dry run passes.
The canary was tested with a command-line Cargo patch to the reviewed source;
no override is committed. Publication and registry lockfile verification must
finish before integrating this branch into dev.

Direct runtime dependencies on `hyper`, `tower-http` and `tower` are removed.
Tower is a dev dependency for existing ServiceExt tests. Axum was already absent
as a direct dependency. Backend libraries remain transitively through the shared
implementation and outbound clients; their lockfile presence is expected.
Logging filters/bridges, Tokio utilities, database drivers and HTTP clients remain
application-owned and used.

The new `tests/e2e_static_files.rs` test runs the configured production frontend
with real loopback HTTP. It checks assets/MIME, HEAD, range and conditional
responses, directory redirects/indexes, SPA/missing-asset fallback, traversal,
unsupported methods, protected API access and successful login. It passes against
both the prior implementation and the shared implementation. Baseline auth and
permission suites pass 44 tests. All-target/all-feature checking passes.

Verification with the temporary source override:

- Full fast-feature unit run: 1,170 passed, two ignored, one scheduler timing
  failure (`test_failed_job_records_error`, 200 ms execution assumption). The
  isolated retry passes; no scheduler implementation/test changes were made.
- All 34 integration suites: 342 passed, 32 existing ignores. The first 11 suites
  passed 145 tests before external removal of the build directory interrupted
  execution. The remaining 23 suites were rebuilt in a separate temporary
  directory and passed 197 tests on the combined CI-fix branch.
- Combined branch frontend/auth/permissions retry: all 45 passed.
- Combined branch formatting, strict production Clippy and locked all-target/
  all-feature checking pass. Integration fixture dead-code warnings and the
  existing num-bigint-dig future-compatibility warning remain.

The registry package has not yet been published or consumed. This branch remains
prepared, not integrated: the lockfile reflects the temporary source override.
Release approval is required before generating and testing the final registry
lockfile and rebasing dev onto this branch.

The migration worktree started at dev `21641eec` and was refreshed onto the
concurrent CI fix `bf9912ae` and Python E2E fix `dc803924`. Their changes are
preserved. The original checkout remains on dev without this pending migration.
