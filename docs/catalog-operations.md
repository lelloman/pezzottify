# Catalog Operations

This runbook covers operational behavior that matters for large SQLite catalogs. It intentionally describes the steady-state design and recovery procedures rather than any single deployment or incident.

## Persisted catalog counts

Artist, album, and track counts are stored in the one-row `catalog_stats` table. Six SQLite `AFTER INSERT` and `AFTER DELETE` triggers maintain the counts and a mutation generation in the same transaction as each catalog change.

Normal startup reads this row in constant time. It does not run `COUNT(*)` across the entity tables. A fresh catalog starts with valid zero counts; an existing catalog upgraded to the stats schema starts with an invalid row so the migration never performs an unbounded scan.

When the row is valid, the counts are exposed as:

```text
pezzottify_catalog_items_total{type="artist"}
pezzottify_catalog_items_total{type="album"}
pezzottify_catalog_items_total{type="track"}
```

When it is invalid, startup remains fast, omits those labeled gauge values, and logs that the manual reconciliation job is required.

## Rebuilding catalog counts

Run the manual-only `catalog_cardinality_stats` background job after upgrading an existing catalog to the stats schema or when repairing suspected count drift. Trigger it from the admin jobs panel or through the authenticated admin API:

```http
POST /v1/admin/jobs/catalog_cardinality_stats/trigger
```

The job:

1. Reads the current catalog mutation generation.
2. Counts artists, albums, and tracks through narrow covering availability indexes.
3. Uses the SQLite progress handler for throttling and cancellation checks.
4. Publishes all three values atomically only if the mutation generation is unchanged.
5. Refreshes the in-process Prometheus gauges after publication.

If the catalog changes during the scan, the job refuses to publish a mixed snapshot and must be retried. A cancellation or process shutdown also leaves the previous stats row untouched; partial counts are never visible.

On catalogs with hundreds of millions of rows, reconciliation can take many minutes and can use substantial read bandwidth. This is acceptable for a rare repair operation, but it should be monitored.

## Monitoring a reconciliation

Observe the job history together with storage and request behavior:

- Job status, duration, and error in the admin jobs panel or job audit API.
- Device throughput, utilization, latency, and queue depth, for example with `iostat -xz`.
- HTTP latency and error rate for foreground requests.
- Tasks stuck in uninterruptible I/O sleep (`D` state).
- Container CPU, memory, and block-I/O counters.
- `/proc/pressure/io` as supporting context.

Do not use a short PSI spike as an automatic cancellation threshold. PSI is host-wide; on an otherwise idle machine it can primarily describe the reconciliation worker waiting for its own I/O. Escalate or cancel when pressure coincides with actual service degradation, growing queues or latency, stuck tasks, or errors.

Use the normal graceful-shutdown path if cancellation is necessary. The job checks the shutdown token through SQLite's progress handler and exits without publishing partial results.

## Steady-state expectations

Once initialized, ordinary inserts and deletes maintain counts transactionally. Routine restarts and catalog reads do not require reconciliation. The manual job is a repair/bootstrap tool, not a scheduled maintenance task.

Related-artist enrichment is a separate queue-backed catalog workload. See [Related Artists Design](related-artists-design.md) for its eligibility, retry, and scheduling behavior.
