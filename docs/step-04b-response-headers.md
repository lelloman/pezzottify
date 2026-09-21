# Step 04b: response header policies

Production cache middleware uses optional `response_headers` operations from
simple-server source `b88b908421db2552ff1e81966c56958925741e27`, recorded in
simple-server.rev for CI/build checkout. The lockfile adds only the shared
crate's direct `http` dependency. Work started from clean active dev `0b258266`
in dedicated branch `migration/step04b-response-headers` and a sibling worktree.

Default no-store still preserves every explicit Cache-Control value. The outer
API layer still covers early auth/CSRF/rate-limit failures only on `/v1` and its
children. Private-cache eligibility, configured max-age, content-type/range
exclusions, ETags, application errors and route/layer placement stay local.
No body is polled or buffered by the shared helpers.

Vary merging now preserves each existing field line and byte, including opaque
values that the former helper could discard. Required names are appended on
separate field lines; case-insensitive duplicate detection and wildcard semantics
are retained. Consumers/tests read all Vary values as a combined list. This is
an intentional representation/preservation improvement, not a new cache policy.
Other response construction, cookies and domain-specific headers remain local.

## Verification

Baseline selected suite: 1,103 library tests, 17 upload/report/streaming HTTP
checks, and 26 catalog HTTP tests pass; two existing library tests are ignored.
Two added middleware regressions also pass against the original implementation
(six cache-policy tests total). 
Final: 1,105 library tests plus the same 43 actual-HTTP checks pass: **1,148 passed,
two ignored**. Coverage includes HEAD, repeated explicit Cache-Control and
Set-Cookie, credential-dependent Vary lists, early auth rejection, API path
boundaries, partial/media/SSE classification, and unchanged body/status/extensions.
Existing actual-HTTP suites cover cache headers on catalog, body-limit failures,
reports, multipart uploads and authenticated media/range streams. Shared tests
cover Vary wildcards/opaque values and lazy data/trailer/error frames.

Commands, run from pezzottify-server with two jobs, debug=0 and target directory
/tmp/pezzottify-03c-target:

- cargo test --lib --test e2e_catalog_tests --test e2e_body_limits
  --test e2e_ingestion_tests --test e2e_reports_tests --test e2e_streaming_tests
- cargo clippy --locked --lib --tests -- -D warnings -A clippy::items-after-test-module
- rustfmt --edition 2021 --check on changed Rust files; git diff --check

Tests used offline resolution for the one dependency-edge lockfile update.
Clippy passes with the previously documented items-after-test-module exemption;
num-bigint-dig retains its pre-existing future-incompatibility notice. Full
frontend/Android, other integration suites, containers and live providers were
not rerun. No new claim about production-sized uploads or deployment is made.

Integration follows the shared workflow: commit here, rebase original dev onto
the migration, verify ancestry and tested tree, remove only this temporary
worktree/branch. Final commit and integration evidence are in simple-server's
trackers. Pre-existing worktrees are retained. No push or deployment.
