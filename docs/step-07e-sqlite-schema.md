# Step 07e: SQLite schema canary

The versioned schema helper delegates table/index creation and schema comparison
to simple-server. The existing rusqlite adapter, descriptor macros and historical
schema arrays remain local. Reviewed shared revision:
`7bc92f5f889dba2fb4c688e12920c3fe248af3a5` (`simple-server.rev`).

All five versioned stores (user, server, catalog, enrichment and download queue)
use shared creation. User, server and download-queue validation uses the shared
comparison report on the existing startup paths. Catalog and enrichment retain
their previous startup validation behavior. Standalone descriptor-based table
creation also uses the shared planner.

## Preserved policy and boundaries

The explicit validation profile preserves exact regular-column count/order,
case-sensitive names, declared types, nullability, first-primary-key-column
flags, and the application's single-layer default-parenthesis normalization.
Named indexes are checked by name; declared table UNIQUE constraints use unordered
sets of named columns from unique indexes; foreign keys compare per-column target
and ON DELETE. Column UNIQUE flags remain creation-only validation exclusions.
Full index definitions, partial predicates, expression terms, foreign-key grouping
and ON UPDATE, generated/hidden columns, CHECK clauses and other DDL properties
are not claimed verified. Extra tables, views and triggers remain allowed.
The shared report explicitly records excluded coverage. Metadata read/decoding
errors propagate instead of silently becoming missing observations.

SQL execution, upgrade transactions, legacy catalog classification, the 99999
version-marker offset, ingestion/search/auxiliary raw schemas, domain queries,
backup and checkpoint behavior remain application-owned. This is adoption of the
versioned helper, not migration of every SQL statement or backup/executor policy.

## Verification

Baseline full library suite: **1,125 passed, two existing ignores**. Final full
library suite: **1,137 passed, two existing ignores**. Local HTTP tests require
loopback permission; the authorized reruns pass. Twelve new canary tests include:

- A frozen test-only copy of the prior creator/validator checks all **38 historical
  snapshots** (user 16, server 8, catalog 10, enrichment 1, download queue 3).
  Both creators and validators interoperate. Actual index term metadata and
  partial predicates are compared independently of name-only validation.
- Mixed-depth legacy acceptance, malformed schemas, expression/partial UNIQUE
  projections, and metadata authorization failure propagation.
- File-backed production server startup upgrades retain data across restart;
  a mid-upgrade collision rolls back DDL/data/markers, then retries successfully;
  schema drift fails before migration or backup registration.

Commands passed:

```sh
cargo test --manifest-path pezzottify-server/Cargo.toml --locked --lib --features test-fast-hasher
cargo test --manifest-path pezzottify-server/Cargo.toml --locked --lib sqlite_persistence::schema_compatibility_tests --features test-fast-hasher
cargo fmt --manifest-path pezzottify-server/Cargo.toml --check
cargo clippy --manifest-path pezzottify-server/Cargo.toml --locked --lib --bin pezzottify-server -- -D warnings
bash scripts/check-db-boundaries.sh
```

Shared schema tests: **22 passed**; full `bash scripts/check` passed, including
strict all-feature lint, feature matrix and documentation. Consumer all-target
Clippy is not claimed (existing test-only lint debt). The existing num-bigint-dig
future-compatibility warning remains. No Docker/browser/Android qualification,
production database access, push or deployment was performed.
