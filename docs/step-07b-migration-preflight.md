# Step 07b: SQLite migration preflight

The production user, server, catalog, enrichment and download-queue stores
inspect their existing `PRAGMA user_version` markers with
`simple_server::database::migrations::MigrationPlan::inspect_version_only`
before executing application-owned schema upgrades. Each store has a separate
namespace and derives its ordered manifest from its existing `VersionedSchema`
array. Fresh-store creation supplies no applied marker. The shared report has
zero historically verified digests because these databases have no migration
history table.

The application still validates user, server and download-queue schema shape
before migration. The catalog still inspects legacy album columns to choose
the effective version and runs its upgrades transactionally. The enrichment
store still creates its auxiliary tables through its existing idempotent path.
The planner does not execute SQL, alter the version marker, or replace backup
and checkpoint behavior.

Ingestion's column-driven idempotent upgrades and the search index's
active/building schema markers are separate layout and rebuild protocols, not
ordered migration ledgers. They remain application-owned; a later shared
SQLite schema module can address reusable shape inspection without inventing
history for them.

Reviewed shared revision: `c1ff3d19d685cd267f5bc712b306f44ef4678d07`
(active pin at rollout). Baseline library suite: 1,120 passed, two existing
ignores. Final suite: 1,121 passed, two existing ignores. The new catalog test
confirms a future version fails before writing. Package formatting and strict
production lib/bin Clippy pass; all-target strict Clippy encounters existing
test-only warnings, including `items_after_test_module` and unused imports.
