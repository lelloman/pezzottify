# Source-backed metadata enrichment

The `metadata_enrichment_v1` job uses the existing typed artist, album and track
tables in `enrichment.db`. New results have `source_status = source_backed_v3`.
Works use the same queue but a separate resolver: see [Works v1](works-v1.md).

## Pipeline

1. Resolve identity using external identifiers, never a bare name.
2. Read structured reference facts directly.
3. For a missing supported field, extract **one fact per LLM request** from a
   short passage fetched for that resolved entity.
4. Validate output and retain per-field provenance. Unknown facts stay null.

Artist identity uses Wikidata Spotify artist ID (P1902) and, when available,
the catalog's MusicBrainz artist ID (P434). Multiple returned Q-IDs are
ambiguous: no first-result selection. Album identity uses MusicBrainz barcode
search; track identity uses ISRC search. MusicBrainz candidates must also match
the normalized title and complete artist-credit names. Multiple matching
candidates or truncated search results do not resolve an identity. Straight and
typographic apostrophes compare equally in titles and credits. Barcode comparison
accepts leading-zero-equivalent numeric GTIN-8/UPC/EAN/GTIN-14 representations;
it does not strip arbitrary punctuation or accept partial codes.

Track-title comparison also ignores one terminal `Remaster`/`Remastered` label,
optionally preceded or followed by a four-digit year from 1900 through 2099.
The label must be in parentheses, square brackets, or after a spaced dash.
The fetched recording must still contain the catalog ISRC and all credited
artists must match. Live, remix, edit, movement, and other version text remains
significant. Album titles do not use this remaster normalization.

Identifier claims use a separate Wikidata qualifier policy: name-as-written
(P1810) and account-quality (P1552) qualifiers are allowed. Temporal and other
uninterpreted qualifiers still abstain. Biographical fact extraction continues
to reject qualified claims.

Resolved IDs are persisted and reused on subsequent attempts. Refreshed records
must still carry the catalog identifier; MusicBrainz titles and credits are also
rechecked. There is deliberately no name-only matching fallback.

## Facts and evidence

Currently supported deterministic facts:

- Artists: explicit person/group type, birth/death/foundation/dissolution dates,
  birthplace or formation place. Citizenship is not converted to origin country.
- Albums: release-group first-release date, label and catalog number. A particular
  edition's release date is not substituted for the album's original date.
- Tracks: title of a single explicitly linked MusicBrainz Work.

Wikidata time precision is preserved (year, month or day). Unsupported calendar
models and coarser dates abstain. Deprecated claims are ignored; preferred claims
take precedence; qualified claims are not flattened into unconditional facts.
Conflicting values remain in evidence rather than being selected arbitrarily.

Missing artist facts can be extracted from the English Wikipedia introduction
linked by the resolved Wikidata entity. Missing album/track facts can be extracted
from MusicBrainz annotations. No arbitrary model-provided URL is fetched.

The extraction field allowlist is deliberately small: artist dates/origin;
album recording start/end dates and label; track composition/recording dates and
language. Biography generation, mood tags, inferred roles and other broad profile
generation are disabled. Expanding coverage requires adding a source adapter or
a separately tested field extractor, not restoring model-memory fallback.

Each completion receives the identified subject, one field and at most 2,500
characters of relevant source text. There are at most two source passages per
field. Responses contain only `value` and `quote`; the quote must occur verbatim
in the fetched passage and contain the value. Dates are normalized by code,
without inventing precision. Empty/truncated completions and extra fields fail
validation. Quote alignment is a guardrail, **not proof of semantic correctness**;
wrong-subject and wrong-field extractions still require factual evaluation.

`entity_evidence_v1` holds individual claims with field, value, source URL,
retrieval timestamp, supporting source data/quote and extraction model where
applicable. An `enrichment_result` evidence row holds the versioned facts,
resolved identities, conflicts and claims needed to resume enrichment.
`entity_external_ids_v1` and `entity_sources_v1` remain populated.

Previously source-backed values are retained when a refresh omits or contradicts
them, together with their original evidence; fresh conflicts are recorded for
review. Legacy deterministic Wikidata fields are preserved, but legacy LLM
fillers are not promoted to verified facts. There is no automatic conflict-review
UI or correction mechanism in this change.

## Queue, failures and configuration

Listening/impression discovery and the normal 90-day stale interval are unchanged.
The former special immediate requeue of Wikidata-only artists for LLM completion
is removed.

Each cycle allows **12 normal attempts, then 3 agent interventions** with the
configured backoff. Another failure ends in terminal `failed_enrichment`.
Missing catalog items terminate immediately. Automatic discovery never reopens
terminal failures or resets pending backoff, counters, or the last error.

Claims reserve items; counters increment only when an item starts executing.
`normal_attempts` and `agent_attempts` track the current cycle; `attempts`
retains its lifetime meaning. Success resets cycle counters. Cancellation
releases unstarted reservations without charging attempts; interrupted work that
actually started retains its charged attempt and audit record.

The agent first retries validated retrieval (so matcher fixes can immediately
clear old backlog), then receives catalog context, known source IDs and prior
attempts. For artist/album/track metadata its actions are `retry_sources`,
`refresh_identity`, or `abstain`. It cannot supply new identifiers, facts, arbitrary URLs or bypass
matching. Refresh rediscovers from catalog identifiers; a change from a previously
verified identity requires review. Unavailable/malformed/truncated agent calls
consume a bounded intervention too, rather than creating another infinite loop.
One intervention may include multiple source requests; this is an attempt budget,
not a count of HTTP calls or extraction completions.

Work-link intervention uses a read-only research loop instead: search MusicBrainz
recordings and works, fetch discovered records and credits, search Wikidata works,
and verify a proposed recording-to-work relationship. Each intervention has at most
5 research rounds, 10 new tool calls, 4,096 output tokens per model call,
120 seconds per model request, and 240 seconds overall. The application advances
an untried source action each round (ISRC, title/artist, discovered recordings and
works, then further work searches). The model can propose additional actions.
Canonicalized duplicate requests are skipped without consuming source calls.
Malformed or truncated responses are recorded and fed into the next round instead
of discarding progress. The outer 12+3 retry policy still applies.

Each round receives a fresh prompt built from accumulated source observations,
fetched evidence, rejected candidates, earlier round failures, untried actions,
and remaining budget. Model prose is never summarized into source facts. Empty
searches, source failures and omitted data remain distinct. Source validation
selects a unique corroborated work and stops early; no final model decision is
required. A model conclusion cannot override validation or abandon an untried
application action. JSON responses can be bare or a single clean code block.

A bounded research checkpoint is saved in attempt diagnostics and restored for
later interventions in the same retry cycle and catalog identity. Successful
searches, including empty results, are remembered. Source errors may be retried
in a later intervention. Full fetched documents are preserved when they fit;
oversized documents are evicted whole, marked as omitted, and can be fetched again.
Compaction never turns a partial document into validation evidence. Manual reopening
starts a new cycle and permits fresh searches. Full current prompt/response/tool
traces remain in the Work evaluation; durable attempt diagnostics keep the compact
checkpoint rather than duplicating those traces within their 64 KB storage limit.
No arbitrary URL, shell or database tools are exposed. A link requires fetched
MusicBrainz evidence with the catalog ISRC/title/artists, one non-composite
performance relationship, and writer credits; acceptance is independent of the model. Wikidata/title/alias matches alone remain unverified.

`agent.llm.intervention_model` optionally selects a separate intervention model,
using the same endpoint and credentials. `intervention_reasoning_effort` selects
that model's supported reasoning policy; extraction-specific reasoning controls
are not carried over to a different model. Model route names are deployment-specific:
verify gateway availability and service permissions before configuring `code:*`.

`enrichment_attempts_v1` retains phase, cycle, ordinal, timing, outcome, errors and
agent diagnostics. Upgrade preserves old lifetime counters in a legacy history
entry and routes outstanding rows with 12+ claims into intervention. Legacy
`failed` rows become terminal. Old counts cannot reconstruct historical
consecutive failures.

An administrator can explicitly reopen up to 50 completed/terminal items through
the existing manual metadata job. This queues a new cycle; the next normal run
processes it. Active cycles are not reset, and previous history is retained:

```json
{"retry_entities":[{"entity_type":"track","entity_id":"TRACK_ID"}]}
```

Unresolved/ambiguous identity, HTTP errors, rate limits and Wikidata API errors
all consume this bounded budget. They never trigger unsourced generation.
If an individual extraction fails, verified facts are saved before the item is
retried; already populated fields do not need another model call.
A legitimate null answer is abstention, not a model error.

Structured retrieval also works with the agent disabled. Extraction uses the
existing shared `[agent.llm]` provider/model and optional `reasoning_effort` and
`thinking_budget_tokens` settings. The current extraction request uses
temperature zero and 1,500 total output tokens. Controls must be supported by the
selected SimpleAI runner; this change does not alter SimpleAI or deployed configs.

Reference HTTP clients use bounded bodies, timeouts and fixed provider endpoints.
MusicBrainz calls share a process-wide throttle of at least 1.1 seconds between
request starts. Retries use the queue backoff rather than tight HTTP loops.

Example manual job:

```json
{"entity_types":["artist","album","track"],"batch_size":5}
```

Deploying does not immediately rewrite every existing profile. Normal stale
discovery/explicitly queued items determine which records are processed.

Job audit records expose `attempted`, `succeeded`, and failure counts.
The legacy `processed` field remains an alias for successful completions.

## Verification

Offline HTTP fixtures exercise identifier resolution/reuse, identity ambiguity,
MusicBrainz corroboration, precision and provider errors. Scripted-model tests
exercise one-field prompts, preservation of known facts, abstention and fabricated
evidence rejection. Work and persistence tests cover external-ID reuse and
namesake separation.

Run from `pezzottify-server`:

```bash
cargo test --lib
```

These tests validate the pipeline, not the factual accuracy of the deployed model.
Before broad rollout, evaluate a reviewed source-backed corpus for identity
accuracy, supported-fact precision, abstention/coverage, latency and false Work
merges. Keep production databases out of those experiments.

## Controlled deployment experiment

Before deploying, set the following in the server configuration (merge into the
existing sections):

```toml
[background_jobs.metadata_enrichment]
manual_only = true
batch_size = 1
work_daily_enqueue_limit = 0

[agent.llm]
intervention_model = "class:fast"
intervention_reasoning_effort = "none"
```

`manual_only` selects `JobSchedule::Manual`; it keeps the job registered and
allows admin triggers without automatic interval runs. Default is false. The
existing pause controls block both scheduled and manual execution: after deployment,
verify that `GET /v1/admin/jobs/metadata_enrichment_v1` reports a `manual` schedule
before clearing any job pause for the experiment. Check global/resource-class
pauses with `GET /v1/admin/jobs/controls` as well. Do not use `interval_hours = 0`
as a scheduling switch. Work discovery is disabled by the zero admission limit;
existing queue entries remain available to an explicit run.

Manual scheduling does not suppress startup migrations. The upgrade still
quarantines legacy unverified Work attachments and migrates retry counters/states.
Capture queue/link/quarantine counts before and after deployment so these changes
are distinguished from experiment results. Ordinary application use can also
create queue entries; manual mode prevents their automatic processing.

Start with one selected track and no catalog/queue mutations:

```http
POST /v1/admin/jobs/metadata_enrichment_v1/trigger
Content-Type: application/json

{"params":{"work_dry_run_track_ids":["09dlwp5vIjt3kavZ1Dgc9d"]}}
```

This is the previously blocked `Two of a Mind - 2003 Remastered` case. The dry run
uses the research path and writes only job audit records, not links or attempt
counters. The offline/live test harness has also verified ordinary source-only
resolution of this item without an LLM. For the server experiment, send one track
per trigger initially. Requests must use the exact `params` envelope shown above;
the generic trigger handler treats a missing/invalid request body as a normal
unparameterized run. The MCP `jobs.action` trigger does not forward custom params,
so use the admin HTTP route for selected-track experiments.

Observe:

- `GET /v1/admin/jobs/metadata_enrichment_v1`: running state, schedule, last run,
  execution policy and circuit breaker state.
- `GET /v1/admin/jobs/metadata_enrichment_v1/history?limit=10`: run status, start/end
  timestamps, manual trigger origin and job-level errors.
- `GET /v1/admin/jobs/metadata_enrichment_v1/audit?limit=50`: per-item start/finish
  progress, dry-run evaluations with exact prompts/responses/tool results, and
  queue-run success/retry/terminal totals. Job completion alone is not proof that
  a link was accepted; inspect `identification.work` or the actual queue/link rows.
- Server logs at INFO: `Work research started`, round starts, tool start/end and
  latency, model response finish reason and reported token usage, duplicate skips,
  failures, and final verification outcome. Entries carry the track ID; model
  failures are WARN. Raw prompts and source payloads stay in the evaluation audit.
- For actual queue runs, `enrichment_queue_v1` contains stage, cycle, normal/agent
  attempt counts, status and last error. `enrichment_attempts_v1` retains per-attempt
  outcomes and compact checkpoints. `work_resolutions_v1.evidence_json` contains
  the latest resolution/research evidence; `work_link_quarantine_v1` retains
  archived legacy attachments. These are available for read-only inspection of
  the enrichment database; there is no dedicated experiment dashboard.

After reviewing the dry run, an actual single-item queue run is:

```http
POST /v1/admin/jobs/metadata_enrichment_v1/trigger
Content-Type: application/json

{"params":{"entity_types":["work_resolution"],"batch_size":1}}
```

This processes the next eligible queued work item, **not a specified track ID**.
`retry_entities` only reopens requested terminal/completed items; it does not
select the next claim or execute them immediately. Inspect the queue before the
write experiment and confirm the claimed item in audit progress. A fresh retry
cycle begins with normal attempts; do not alter counters to force intervention.
An agent-stage item can be observed when it naturally becomes eligible.

Stop a running experiment with
`POST /v1/admin/jobs/metadata_enrichment_v1/cancel`. Leave manual-only mode enabled
between runs. Stop expanding the experiment on unsupported accepted links,
unexpected queue claims, repeated source requests, missing audit/checkpoint data,
or database timeout errors. Each research intervention is bounded to 240 seconds,
but normal lookups and database operations are separate. Existing database
executor timeouts remain unresolved; database-backed audit writes can fail too,
so retain server logs and reconcile them with persisted outcomes. Full streaming
prompts/checkpoints are not persisted after every model call: cancellation before
an item finishes can lose its in-flight detailed trace, while server logs show its
last reported step.
