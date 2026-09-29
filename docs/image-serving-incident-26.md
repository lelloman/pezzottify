# Image burst admission: LLPR/PEZZOTTIFY-26

Ticket: [LLPR/PEZZOTTIFY-26](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/26)

## Production evidence (29 September 2026)

Read-only investigation verified the running container image
`sha256:04ea74919e664aa17e0f73f481b64ba426d3ef91bfbc4f682932c603493db093`.
Its OCI revision is `be54a330`, built 18 September. Its image-serving code and
filesystem pool match the pre-fix checkout: four workers, one-second admission
timeout, 60-second execution timeout. These limits are compiled defaults, with no
TOML overrides. The media volume is `/mnt/external/homelab/pezzottify-catalog`,
on ext4 `/dev/sdb1`, a rotational External USB 3.0 disk.

Prometheus queries at `1790665200` (07:00 UTC), over the preceding 15 minutes:

- `sum by(path,status)(increase(pezzottify_http_requests_total[15m])) > 0`
  returned image-route increases of 88.988764 for 503 and 30.337079 for 200:
  74.58% errors.
- `increase(pezzottify_blocking_work_operations_total[15m])` returned
  95.056180 filesystem queue timeouts.
- `increase(pezzottify_blocking_work_execution_seconds_bucket{pool="filesystem"}[15m])`
  returned 196.179775 total, 186.067416 at <=1s, and 192.134831 at <=5s.
  Approximately four operations exceeded five seconds.

At `1790664540` (06:49 UTC), the one-minute increases of
`node_disk_read_time_seconds_total{device="sdb"}` and
`node_disk_reads_completed_total{device="sdb"}` were 58.004 seconds and 58
reads, respectively: approximately one second per completed disk read.
Prometheus increases are extrapolated values, not exact request counts.

Container logs show the image burst at 06:48:48, repeated 503 responses after
1.0–1.055s, and four 200 responses at 06:48:55 taking 7.121–7.138s. Later
image reads complete in 5–91ms, while cache persistence warnings also appear.

This establishes slow disk I/O and a queue deadline shorter than the observed
stall as the immediate failure mechanism. The old shared pool also allows
publication, recovery, and reads to starve one another. Historical metrics lack
per-operation labels: they cannot prove which operation occupied each worker,
or distinguish disk wake-up from other causes of the storage stall. We did not
spin down the production disk, drop its caches, or change its configuration.

## Change and resource limits

Image pointer lookup and local byte reads execute together in a dedicated
`image_read` pool. The safe filesystem adapter and image validation are retained.
Publication, recovery, audio operations, and other filesystem work retain the
existing `filesystem` pool. Cache publication remains durable and best effort
after a successful upstream fetch.

- Four concurrent image operations; four existing general filesystem workers.
  Aggregate capacity is at most eight blocking closures across these two pools.
- At most 128 admitted image operations, including executing and queued work
  (normally at most 124 queued). The incident burst was roughly 30 simultaneous
  requests; the regression uses 64. The admission cap provides headroom without
  an unbounded waiting queue.
- Ten-second image queue deadline accommodates the measured ~7s disk stall;
  the existing 60-second execution deadline is unchanged. This is not a guarantee
  for indefinitely stalled storage: sustained overload still fails explicitly.
- Full admission, queue timeout, and execution timeout return HTTP 503 with
  `code=filesystem_busy` and `Retry-After: 1`.
- Executing closures keep both permits until they actually stop, even after
  caller cancellation or execution timeout. Cancelled/expired waiters release
  admission. A timed-out blocking system call is not forcefully interrupted.
- The admission cap covers local image I/O, not the whole HTTP response lifetime
  or upstream fetches. Existing image byte-buffer and upstream behavior is unchanged.

Existing blocking-work metrics now distinguish `pool="image_read"` and
`outcome="queue_full"`; queue-wait histograms include a ten-second bucket.
Use image pool queue timeouts/full rejections, execution duration, and route/status
success rates together to assess continued storage trouble.

## Validation and rollout

The deterministic regression `image_burst_survives_cold_disk_and_competing_filesystem_work`
compares the old shared-pool setup (64/64 queued reads rejected) with the dedicated
pool. Four general filesystem operations remain occupied while four image
workers simulate a seven-second disk stall, followed by 64 distinct local images.
The test requires 64/64 successful images before ten seconds and no image queue
timeouts. Separate tests cover full admission, cancellation, execution timeout,
permit retention, and the retryable HTTP error contract. Existing media tests
cover upstream fallback, validation, publication failure, and safe file access.

Validation on 29 September 2026:

- Burst reproduction: 64/64 successes in 7.010s, zero image queue timeouts;
  shared-pool control: 64/64 rejected.
- `cargo test --features fast`: 1,457 passed across 38 suite results, 36 ignored
  (including opt-in integrations/doc examples). This includes 1,125 unit tests.
  The first sandboxed run could not bind local HTTP sockets; the unrestricted
  rerun passed all enabled tests.
- `./e2e/run.sh tests -m 'not android'`: 45 passed, two Android tests deselected.
- `cargo fmt --check`, `bash scripts/check-db-boundaries.sh`,
  `cargo clippy -- -D warnings`, `cargo build --release`, and `git diff --check`
  passed.
- `cargo audit` exited successfully with five existing allowed warnings
  (`number_prefix`, `anyhow`, two `rand` versions, and yanked `spin`). This change
  does not modify dependencies.
- The local simple-server checkout differs from `simple-server.rev` only in four
  documentation files; dependency source and manifests match the pinned revision.

Production deployment and recovery must be recorded separately from this local
reproduction. The checkout is ahead of the deployed revision; deploying its HEAD
would also ship unrelated changes. For an incident-only rollout, apply the scoped
commit to `be54a330`, build that revision, and retain the image digest above for
rollback. Verify the new revision before checking representative image traffic.

After rollout, compare image-route status counts and
`pezzottify_blocking_work_operations_total{pool="image_read"}` before/after a
bounded image burst. Observe at least one naturally cold-storage window as well
as warm reads. Zero traffic or a NaN error ratio does not establish recovery.
