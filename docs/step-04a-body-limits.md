# Step 04a: body limits

The three production extractor-limit declarations use the optional shared
`body_limit::BodyLimit::max` interface: legacy bug reports retain 2 MiB, reports
retain 20 MiB, and ingestion multipart retains 5 GiB. Their route placement,
auth/admission order, rejection handling and application-specific upload limits
are unchanged. No global limit or response buffering is introduced.

Reviewed source: `0b945750b6b97a9e18c531d1cf1137d4ed4b69c9`, recorded in
simple-server.rev for existing CI/build checkout scripts. The lockfile adds only
the shared crate's tower-layer dependency. DefaultBodyLimit is no longer imported
or used by production code.

## Verification

Started from clean active dev `69988032` in dedicated branch
`migration/step04a-body-limits` and sibling worktree. Baseline library suite:
1,103 passed, two existing ignores. Before migration, the new actual-HTTP canary
and existing ingestion/reports/streaming suites pass (17 tests). The canary
fixture initially used an admin lacking ReportBug permission; it was corrected
to the existing report-authorized user without changing production behavior.

Final: 1,103 library tests and the same 17 HTTP tests pass (1,120 total, two
existing ignored library tests). The new real-HTTP test exercises below/at/above
2 MiB and 20 MiB JSON ceilings, no-store behavior and a 3 MiB multipart file that
reaches filename validation, proving the large-upload exception remains active.
An empty filename avoids creating ingestion jobs. The full 5 GiB boundary is
preserved by code review, not a multi-gigabyte runtime upload. Shared-library
contract tests cover exact multipart boundaries and absent Content-Length.
Existing suites verify report permissions/quota/response contracts, ingestion
routing and eleven authenticated full/partial/range-streaming cases.

Commands: cargo test --locked --lib --test e2e_body_limits
--test e2e_ingestion_tests --test e2e_reports_tests --test e2e_streaming_tests;
cargo clippy --locked --lib --tests -- -D warnings -A clippy::items-after-test-module.
Builds use two jobs, /tmp/pezzottify-03c-target and debug=0. Clippy passes with only
the previously documented items-after-test-module lint exempted; dependency
num-bigint-dig retains its existing future-incompatibility warning. Changed-file
rustfmt and whitespace checks pass. Other integration suites, frontend/Android,
containers, provider workflows and production-scale uploads were not rerun.

The original dev branch is rebased onto the migration; ancestry and identical
tested tree are verified before removal of the temporary branch/worktree.
Integration revision is recorded in the simple-server trackers. No push/deploy.
