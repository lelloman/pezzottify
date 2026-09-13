# External downloader status reporting

Deploy the Pezzottify server before updating `scripts/cron_downloader.py` in
pezzottify-downloader. Older servers lack the reporting endpoint; the new worker
will refuse to download if its start report is rejected.

The cron worker retains its existing process lock, cooldowns, and full-album
upload policy. This API reports its activity; it is not a distributed worker lease.

`POST /v1/download/admin/request/{id}/attempt` requires `EditCatalog` permission.

- Before fetching metadata, send `{"status":"IN_PROGRESS","previous_attempt":null}`,
  replacing null with the queue item's `last_attempt_at` when present.
- The response contains an integer `attempt` token. It is a monotonically
  increasing attempt timestamp, also exposed as `last_attempt_at`.
- On failure, send `{"status":"FAILED","attempt":123,"error_message":"Missing tracks"}`.
  Error messages must contain 1–4096 UTF-8 bytes.
- The server rejects stale queue snapshots or failure reports with HTTP 409.
  Completed requests cannot be restarted or failed by this endpoint.
- Successful uploads remain in progress until ingestion marks them completed.
  Workers cannot report completion through this endpoint.

Initial attempts keep `retry_count=0`; subsequent attempts increment it. The
external cron retains control over retry scheduling, as before. Attempt starts
and failures are recorded in the server audit log.

Dry runs do not change server state. Reports refresh authentication once on HTTP
401. If a final failure report cannot reach the server, the worker logs that
reporting error separately and keeps its local failure record. The server may
remain IN_PROGRESS until the next attempt; reports are not durably replayed.
Existing pending requests are corrected on their next attempt, not backfilled.
