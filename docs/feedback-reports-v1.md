# Feedback reports v1

Implementation contract for issue PEZZOTTIFY-15.
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
| GET /v1/admin/reports/{id}/notes | Triage-only notes, cursor-paginated; excludes diagnostic/audit events |
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
General report audit storage is capped at 2,000 events/resource and 200,000 globally, also subject
to the aggregate metadata-byte ceiling. One event and 512 bytes are reserved per live
attachment for owner deletion. Other general report audited operations fail closed at capacity. Automation uses
the independent bounded retention described below.
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

## Android flow

Settings opens the feedback form; each assistant response also has a report action
that selects only the recorded turn correlated with that message. Recording is off
by default and opted into separately for each server/account. It captures future
activity, not a retroactive import of chat history. The diagnostic file lives in
Android `noBackupFilesDir`. Recording failure does not interrupt normal chat.

Both upload attachment switches start off. Review freezes metadata and attachment
bytes; preview displays those same bytes and a separate Send action submits them.
Retry is manual and reuses the same request UUID and snapshot. Editing creates a
new snapshot/key. Account or recorder invalidation clears unsent drafts/previews.
Uploads bind URL and credentials to one account state and exclude general HTTP
logging/interceptors and redirects. An already dispatched upload may still arrive
at its original server; it is never retargeted to a newly selected account.

The form shows submitted report IDs and a refreshable, paginated owner status list.
English and Italian UI explain local deletion versus already submitted reports,
size/quota/capacity errors, and privacy limitations. Builds use the source revision
in `simple-android-assistant.rev`, provisioned by
`scripts/checkout-simple-android-assistant.sh`, with an explicit
`-PassistantCheckout=/path/to/checkout` override for development. Composite dependency
substitution avoids requiring publication of the historical JitPack pin.
See [qualification and rollout gates](feedback-qualification.md) for test coverage,
remaining Android/device checks and the separately authorized rollout sequence.

## Controlled automation v1 implementation

Server schema v9 adds `report_integrations` and `report_deliveries`; upgrading
preserves existing reports and creates **no destinations**. Migration has no
backfill or historical replay. Back up `server.db` before upgrading. Signing keys
are stored in that database and its backups, which require the same protection
as existing diagnostic data. There is no deployment or external destination setup
in this change.

The integrations collection returns an array of `{id,url,enabled,version}`.
`POST` accepts only `{url}` and returns 201 with `{integration,secret}`: `secret`
is a randomly generated 32-byte key encoded as standard base64, returned once.
Decode that base64 before verifying HMAC. `PATCH /integrations/{id}` accepts
`{expected_version,enabled}`. URLs and keys are immutable; creating another
disabled integration is required for a new receiver. All ten slots include disabled
integrations. No deletion/rotation endpoint is provided in v1.

Only `submitted` (including legacy submission) and `updated` events enqueue
outbound work. Their audit event UUID is the outbound `event_id`; every destination
has its own stable delivery UUID. Each JSON body contains `schema_version:1`,
`event_id`, `event_type`, `created_at` (Unix seconds), and
`report:{id,kind,category,status,version}`. The exact serialized body is persisted
with the audit event and report mutation in one transaction, so retries retain
identical bytes. Reads, notes, attachment access/deletion and administrative audit
events never enqueue work. Existing immutable events may still describe reports
that were subsequently changed or deleted through legacy administration.

The worker starts with report admission, claims at most one delivery at a time,
and performs network work outside database transactions/executor slots. A claim
consumes an attempt and grants a 60-second lease with a unique token; expired
leases recover after crash and late completions cannot overwrite another claim.
One DNS-plus-HTTP attempt has a ten-second deadline. DNS is resolved on every
attempt, every result must be globally routable, and at most 64 results are
accepted. The resulting addresses are pinned into a fresh HTTPS client. Redirects
are terminal for that attempt and are never followed; environment proxies are
disabled. Non-global/special-purpose IPv4 and IPv6, mapped IPv4, transition ranges
and mixed public/private DNS answers fail closed. This intentionally rejects some
special-purpose ranges conservatively. Only body bytes up to 4 KiB are accepted;
remote bodies and error text are never saved or logged.

All HTTP statuses are recorded as `http_<status>`; only 2xx succeeds. Other outcomes
are bounded reason codes (`destination_rejected`, `transport_error`,
`response_too_large`, `deadline`, `lease_expired`). Logs carry delivery/event IDs
and the same outcome. Retries use 30, 60, 120 and 240-second backoff for five total
automatic attempts. A 429 `Retry-After` delta or HTTP date can increase that delay,
capped at one hour. After failure, each explicit retry grants **one** additional
attempt; at most five manual retries are permitted (ten lifetime claims). Crash
recovery also consumes this budget. Disabling an integration prevents future
claims, including retries; an in-flight request cannot be recalled. Re-enabling
resumes pending work.

`GET /deliveries` accepts `before`, `limit` (1–100, default 25), `state` and
`integration_id`, and returns `{items,next_cursor}` in descending sequence order.
Items expose stable IDs, current state, attempt/manual-retry counts, next scheduled
Unix timestamp, version and last outcome, without payloads or keys. This is current
state inspection, not an unbounded per-attempt history. `POST /deliveries/{id}/retry`
accepts `{expected_version}`, requires a failed delivery with remaining manual
budget, and returns JSON null. Stale versions/invalid states/exhausted budgets
return 409. Report stats include pending/leased/succeeded/failed delivery counts.

The hard bounds are ten integrations, 10,000 total retained deliveries (including
terminal records), and 256 KiB per outbound event. Terminal delivery records older
than 30 days are pruned when new eligible work is enqueued; pending and leased work
is never evicted. These limits bound queue storage even if delivery fails. At
capacity an eligible report mutation fails with the existing structured 503 and
rolls back completely, including its audit event; exact idempotent submission
replay still succeeds. Owner diagnostic deletion never depends on outbox capacity.
All integration/delivery reads and control mutations require
`ManageReportIntegrations` and are audited under
`/v1/admin/reports/automation/events` (inspection of that audit trail separately
requires `ViewReportDiagnostics`). Automation auditing uses separate storage from general report audit/metadata quotas.
Inspection and control each retain the newest 1,000 records, for at most 30 days;
pruning runs atomically on every automation audit write. Polling cannot evict control
records or exhaust the disable budget. At capacity, the oldest records in that
partition are evicted, including for manual retries and enable/disable operations.
Records have bounded resource IDs (128 bytes), kinds (64 bytes), and JSON data
(256 bytes): at most 2,000 records and under 1 MiB of serialized text, independent
of report metadata limits. Storage failures still roll back the operation and audit
together. The `automation` audit cursor is independent of `administration`, which
continues to hold legacy deletion audit records.
Signing keys, URLs, remote bodies and report text are absent from delivery logs
and administrative audit data. The existing report body admission limits,
frequency controls, sensitive-body logging exclusions and no-store responses also
apply to these endpoints.

## Web flow

Settings contains account-scoped local recording consent and the feedback form.
The assistant's “Report a response” controls select only the turn correlated with
that reply; including all retained turns is a separate explicit choice. Recording
starts with future activity and never imports existing model history. Tool
confirmations/results, cancellation, compaction, versions and partial streams are
recorded separately from the assistant archive. IndexedDB keeps a bounded v1
recording and recovers running turns as interrupted. Transactional clear epochs
reject writes from another tab that predates a clear/account change.

Both attachment checkboxes start off for each form. Technical logs can be pasted
into a separate field; there is no global console or HTTP-body collector. Review
freezes the complete request string and UUID. Send/retry uses that string without
collecting newer activity, and retries are manual. Editing creates a new UUID.
The form displays the receipt and a refreshable, paginated owner status list.

Requests use native fetch with no-store and redirects disabled, outside general
API retry/interceptor handling. Before dispatch, the explicit OIDC or password-login
bearer credential is checked against the current owner at `/v1/auth/session`, then
that same credential is used with cookies omitted. This prevents a cookie/account
switch in another tab from retargeting a report. A session without an available
explicit credential must sign in again; the client does not fall back to ambient
cookies for report content. Clear, disable and account changes invalidate previews
and abort pending requests; already dispatched reports may still reach the original
account. Cross-tab clear/account notifications invalidate local previews too.

Browser persistence is best effort. Storage failures are visible and nonfatal to
chat, and an evicted recording is treated as absent with recording off. No automatic
upload, attachment forwarding, or deployment is part of the web flow.

## Web administration

The existing Feedback Reports section uses the versioned APIs and accepts any of
TriageReports, ViewReportDiagnostics or ManageReportIntegrations for navigation.
Each capability has independent controls and requests. Diagnostic-only operators
can inspect/delete an attachment by known report/attachment IDs and inspect audit
resources without permission to browse report metadata. Notes have a separate
triage-only paginated endpoint; owner detail/list responses never include notes.
All transcript fields, including tool arguments/results, are inert plain text.

Integration controls list existing destinations and confirm enable/disable and
single-attempt retry. Creating destinations/obtaining signing keys remains an API
administration operation; this UI never creates a live destination or stores keys.
Limits/retention updates require the complete current version and confirmation.
Conflicts require an explicit refresh and review rather than automatic replay.
