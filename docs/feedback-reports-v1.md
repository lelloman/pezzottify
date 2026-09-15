# Feedback reports v1

Implementation contract for [LLPR/PEZZOTTIFY-15](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/15).
This document specifies target behavior; a checked-in contract does not imply that
every endpoint or client is already implemented. Delivery evidence belongs on the tickets.

## Principles and ownership

Extend existing bug reports; do not create an unrelated upload system. Report
metadata survives diagnostic expiration. Existing `/v1/user/bug-report` clients
remain supported through an adapter. Legacy admin APIs must not bypass permissions
or auditing. No production deployment, live integration configuration, or external
publication is authorized by this implementation.

Record locally, upload only on explicit submission. Local assistant recording is
disclosed and switchable; disabling deletes the recording. Scope storage to server
identity and authenticated user. Logout/account change clears it and any unsent
diagnostic snapshots. Clear chat clears the diagnostic copy too. Exclude Android
diagnostics from backup. Browser persistence is best effort (IndexedDB can be evicted).
Already submitted reports are not removed by local clear; explain this in the UI.

## Diagnostic envelope

`schema_version: 1`, `captured_at` (UTC RFC3339), and ordered `turns`. Each turn has
`id`, `conversation_id`, `started_at`, `status` (running/completed/failed/cancelled/
interrupted), and ordered `events`. Events have `kind`, `timestamp`, optional
`content`, `tool_name`, and `data`. Kinds: user, assistant, tool_call, tool_result,
confirmation, error, cancellation, compaction, metadata. Data contains correlation
IDs, provider/model and versions, timing, and structured tool arguments/results.
Never collect hidden reasoning, HTTP bodies/headers, credentials or configuration
secrets. Recursively redact credential-like keys and recognizable bearer/JWT/key
values before writing. Free text may still contain personal data: redaction is not
a guarantee of anonymity; show a warning and the actual attachment preview.

Cap **compact UTF-8 JSON including envelope/record metadata** at 2,097,152 bytes.
Evict oldest complete turns; truncate an oversized newest turn with an explicit
marker, preserving IDs/status. Never split a UTF-8 code point. Persist partial
responses periodically, not a token-by-token duplicate transcript. Mark running
turns interrupted on recovery. Serialize mutations through one writer and reject
stale account generations. Recorder failure is non-fatal to chat and never alters
model context, normal history, or execution behavior.

Snapshot only the selected conversation/turn by default; including other retained
turns requires an explicit choice. Snapshot before preview; preview and upload must
refer to the same immutable bytes. Consent is per attachment, conversation off by
default. Unsent snapshots are local/account-scoped, never background uploaded.

## Resources and HTTP contract

All routes require current session authentication. New API JSON uses snake_case.
Reports use stable UUID IDs and monotonically increasing `version` for concurrency.
Kinds: `bug`, `feature`. Categories: assistant, playback, downloads, ui, other.
Statuses: new, investigating, planned, resolved, closed; admins may reopen reports.

| Route | Semantics |
| --- | --- |
| POST /v1/reports | Submit; return 201 with report ID/version; exact idempotent replay returns 200 |
| GET /v1/reports | Owner-only summary listing |
| GET /v1/reports/{id} | Owner-only metadata and attachment descriptors; no internal notes |
| GET /v1/reports/{id}/attachments/{attachment_id} | Owner fetch of own attachment |
| DELETE /v1/reports/{id}/attachments/{attachment_id} | Owner deletion of own sensitive attachment |
| GET /v1/admin/reports | Filterable administrative summary listing |
| GET/PATCH /v1/admin/reports/{id} | Inspect/update status, internal note or duplicate link |
| GET /v1/admin/reports/{id}/events | Cursor-paginated administrative audit trail |
| GET/DELETE /v1/admin/reports/{id}/attachments/{attachment_id} | Audited diagnostic access/deletion |
| GET/PUT /v1/admin/reports/settings | Effective limits/retention, version-checked updates |
| GET /v1/admin/reports/stats | Storage, report counts, rejections and delivery state |
| GET/POST /v1/admin/reports/integrations | List/create disabled-by-default outbound destinations |
| PATCH /v1/admin/reports/integrations/{id} | Version-checked enable/disable/update |
| GET /v1/admin/reports/deliveries | Cursor-paginated delivery attempts/state |
| POST /v1/admin/reports/deliveries/{id}/retry | Version-checked explicit retry, bounded attempts |

Create fields: client_request_id (UUID), kind, category, optional title, description,
client_type, optional client_version/device_info, and attachments. Each attachment
has `kind` (technical_logs/assistant), `content` (UTF-8 string) and `consent: true`.
Assistant content must validate as a v1 envelope. No archives or arbitrary URLs.
Image upload remains a legacy capability, not a new UI requirement for v1.

Idempotency is `(authenticated user, client_request_id)` plus canonical payload
hash. Same key/different payload is 409. Retry does not duplicate attachments,
events, quotas or deliveries. Keys last as long as report metadata. PATCH requires
`expected_version`; stale updates are 409. Notes and duplicate changes are audited;
reject self-links/cycles/nonexistent targets. Ownership failures return 404.

List filters: kind, category, status, client_type, client_version; admin also user_id
and has_diagnostics. Order by descending immutable sequence; cursor is exclusive
`before` sequence. `limit` defaults 25, maximum 100. Return `items,next_cursor`.
Summary endpoints never embed diagnostic content. Detail attachment descriptors
include ID, kind, UTF-8 size, creation/expiration time, and deletion/expiration state.

Error envelope: `{"error":{"code":"...","message":"..."}}`. Invalid input 400;
unauthenticated 401; forbidden 403; unknown/not-owned 404; conflicting version/key
409; oversized body/attachment 413; unsupported content encoding 415; quota 429
with Retry-After; global capacity 503 with Retry-After. No credentials or raw SQL
in errors. Responses use no-store. Exclude request AND response bodies for all report
routes from general HTTP logs regardless of logging level.

## Permissions

`ReportBug` permits submission/owner routes (existing permission). `TriageReports`
permits metadata, notes, status, duplicates and aggregate stats. `ViewReportDiagnostics`
permits sensitive attachment reads/deletes and audit inspection. `ManageReportIntegrations`
permits integrations/settings/delivery management. Admin role receives all three;
explicit grants work without ServerAdmin. Every resource access enforces permission
and ownership anew; UI hiding is not authorization. Record actor/time/action/resource
for sensitive access, mutation and integration changes, never attachment contents.

## Limits (defaults; configurable downward/upward within hard ceilings)

| Resource | Default / hard ceiling |
| --- | --- |
| New request wire body | 20 MiB (allows JSON escaping of 3 MiB content); never raised |
| Description / title | 100 KiB / 200 UTF-8 bytes |
| Technical logs / assistant | 1 MiB / 2 MiB; at most one each |
| Client metadata | 4 KiB combined; enum client_type web/android |
| Concurrent request-body reads | 2/user, 8/server; bounded read deadline 30 seconds |
| Submission attempts | 30/user/minute, plus existing IP protection |
| Accepted reports | 5/user/hour, 20/user/day |
| Accepted attachment bytes | 32 MiB/user/day |
| Retained metadata | 1,000 reports/user, 100,000 globally; 256 MiB aggregate text |
| Active diagnostic storage | 500 MiB globally |
| Diagnostic retention | 30 days, configurable 1–90 days |
| Integration count / pending deliveries | 10 / 10,000 globally |
| Delivery attempts / HTTP deadline | 5 attempts / 10 seconds |
| Delivery response / event payload | 4 KiB / 256 KiB |

Authenticate and acquire bounded body-reading slots before buffering/parsing. Enforce
wire size incrementally, not only Content-Length. Reject content encoding other than
identity. Hold quota checks/reservations and insertion in one DB transaction, including
legacy submissions. Count SQLite TEXT using `length(CAST(value AS BLOB))`, not characters.
Do not silently drop user-selected attachments: reject with an actionable capacity error.
Expire attachments without deleting report descriptions/status. Periodic expiration
and opportunistic cleanup share one transactional implementation. Operational counters
use bounded reason labels, never user IDs or descriptions as metric labels.

Backend implementation details: settings updates submit the complete settings object
with its current `version`. Count/byte defaults above are also hard ceilings; admins
can lower them and subsequently restore them. Retention can range from 1 to 90 days.
Shortening retention shortens existing expiry deadlines; increasing it never extends
an existing attachment's deadline or resurrects deleted content. Expiration runs on
startup, every 60 seconds, and during new submissions. Historical database backups
remain subject to the server's separate backup policy; expiration is not secure erasure.

Report operations are additionally bounded to 120 requests/user/minute; admission
keeps at most 4,096 user windows and holds slots through parsing and handler execution.
Non-submission report bodies are limited to 128 KiB (including JSON escaping).
Audit storage is capped at 2,000 events/resource and 200,000 globally, also subject
to the aggregate metadata-byte ceiling. One event and 512 bytes are reserved per live
attachment for owner deletion. Other new audited operations fail closed at capacity.
`GET /v1/admin/reports/settings/events` and `/v1/admin/reports/administration/events`
expose settings changes and legacy whole-report deletion requests with diagnostic
permission. Legacy whole-report deletion retains a separate deletion-request audit
record; it is not evidence of successful deletion. Stats include effective settings,
current storage usage and bounded HTTP rejection counters since process startup.
Outbox/queue limits are implemented with the automation task, not by upload admission.

## Automation and external sharing

Write report events and outbox entries in the same transaction. Worker claims leased
deliveries, recovers expired leases after crash, retries with bounded exponential
backoff, and records terminal failure. At-least-once, never claim exactly-once; receiver
deduplicates stable event UUIDs. Signature: HMAC-SHA256 over timestamp + '.' + exact body,
headers X-Pezzottify-Event-ID, X-Pezzottify-Timestamp, X-Pezzottify-Signature (v1=hex).
Secrets are generated/returned once, never listed/logged. Honor 429 Retry-After within
a bounded scheduling horizon. Retry is explicit after exhaustion and remains capped.

Outbound envelope v1 contains only event ID/type/time and report ID/kind/category/status/
version by default: no free-text description, user identity, notes or diagnostics.
No diagnostic forwarding in v1; a later explicit per-destination user-consent contract
is required before extending payloads. Destination admin-only, HTTPS only, no userinfo/
fragment, no redirects/proxy environment. Resolve DNS on every attempt and reject all
non-global IPv4/IPv6 addresses; pin vetted DNS addresses into the HTTP connection to
prevent rebinding. Reject metadata endpoints, loopback/private/link-local/multicast and
IPv4-mapped bypasses. Disable destinations immediately for future claims; in-flight
delivery cannot be retracted. Never fetch links appearing inside a report.

## Admin and delivery acceptance

Expose filters, versions/conflicts, statuses, internal notes, duplicate links, safe
plain-text transcripts, attachment sizes/expiry/deletion, storage capacity/rejections,
audit trail and integration/delivery control. UI respects the permission matrix and
surfaces actionable 409/413/429/503 errors. User screens show report IDs/status and
manual retry without collecting live chat after consent. No voting, discussions,
automatic GitHub issues, model replay, or execution of uploaded tool calls.

Validation includes shared Unicode/redaction fixtures, exact-byte boundaries,
oversized turns, crash recovery, stale writes after clear/logout, unchanged snapshot
uploads, auth/ownership matrix, concurrent idempotency/quotas, retention, SSRF and
delivery crash/retry tests, migration/legacy compatibility and both client builds.
