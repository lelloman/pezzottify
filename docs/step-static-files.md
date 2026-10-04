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
The canary is being tested with a command-line Cargo patch to the reviewed source;
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

The first broad fast-feature unit run passed 1,170 tests with two ignored, but
`test_failed_job_records_error` failed its 200 ms execution assumption while other
builds/tests were running. An isolated retry passes. HTTP integration suites are
still being completed; this record must be finalized before integration.

The migration worktree starts at dev `21641eec`. Concurrent unrelated changes in
the original checkout, including test-fixture formatting, must be preserved.
