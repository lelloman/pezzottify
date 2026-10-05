# Works v1

A Work identifies a composition or other single performable unit, independently
of its recordings. Tracks linked to the same external Work share a stable UUID.
Textual identity is secondary to external identifiers. A movement or aria has its own Work;
its title must identify its parent composition and movement. Work identity does
not include the performer, album, live/remaster suffix, or ISRC.

## Queue and resolution

The `metadata_enrichment_v1` background job now accepts `work_resolution` as an
entity type. Its queue entity ID is a **track ID**, not a Work ID. Work resolution
is independent of ordinary track metadata enrichment.

The initial interpretation of “known track” is a catalog track with local audio
available (`track_available = 1` and a non-null audio URI). On every run that
includes Works, discovery runs **independently of queue processing**, even
when other enrichment or retries are pending. It admits up to the smaller of
`batch_size` and the remaining `background_jobs.metadata_enrichment.work_daily_enqueue_limit`
(default **400 new requests per UTC day**). Set the daily limit to zero to pause
discovery without disabling processing or retries. This is an admission budget,
not a promise of 400 successful resolutions per day. Manual runs share the budget.
When an existing batch has already been claimed, newly discovered requests are
available to subsequent runs. When no batch was eligible, new requests can be
claimed immediately, alongside listening backfill.
Persisted queue creation timestamps count requests in all statuses; retries do
not consume it again, and server restarts do not reset it. The budget resets at
00:00 UTC. Existing queue history counts toward the first day's budget on upgrade.

Each scan examines at most 5,000 rows and persists its offset, without queueing
the full catalog. At the end of the catalog it wraps around,
so newly available tracks are eventually picked up. Existing queued, running,
failed, or completed resolutions are not automatically reset. Normal listening
metadata keeps its higher priority. A manual run can isolate resolution:

```json
{"entity_types": ["work_resolution"], "batch_size": 25}
```

Resolution is source-first:

1. Look up the recording by ISRC in MusicBrainz, corroborating its title and
   complete artist credits. Recheck the identifier on the fetched recording.
2. If that recording has a single explicit performance-to-Work relationship,
   use the referenced Work and its writer/composer/lyricist/librettist credits
   directly, without an LLM. Missing creators, partial/medley relationships and
   multiple Works remain unresolved.
3. Normal attempts that cannot establish a single explicit relationship retry
   within the shared 12-attempt budget. Afterward the bounded intervention agent
   can search MusicBrainz recordings and works, fetch credits and relationships,
   and search Wikidata candidates in a bounded tool loop. A title-based model choice is
   retained in research evidence only, never as a verified Work attachment.
   Even a fetched Q-ID with matching copied fields does not establish that this
   recording performs that composition. Contradictory explanations cannot create
   links because the final proposal must pass an independent source validator.
4. Reuse/create the local Work only through the explicit MusicBrainz relationship,
   retaining its evidence. Different externally identified Works do not merge
   merely because title/creator text matches.

Uncertain tracks keep an unresolved result and consume the bounded retry budget;
after three failed interventions the queue becomes `failed_enrichment`.
Already completed unresolved evaluations from older versions are not bulk
requeued by this upgrade. An explicit manual retry can reopen them.

On startup, the schema upgrade archives existing `wikidata_supported_v1`
attachments in `work_link_quarantine_v1`, including their original Work IDs,
status, reason, evidence and timestamps. It then detaches those unverified
recording links, marks their resolutions `needs_review_v1`, and marks their
queue rows terminal. The underlying Work records and source identifiers are
retained. This is a reversible quarantine of an unverified mechanism, not a claim
that every old link was wrong. Explicit MusicBrainz links are unaffected.
The original evidence remains available for review even after manual retry.

Work identity normalization preserves accents, punctuation and movement
identifiers. Recording matching separately folds typographic apostrophes.
It also tolerates one explicit terminal remaster label, such as
`Two of a Mind - 2003 Remastered` versus `Two of a Mind`, while still requiring
the same fetched ISRC and complete artist credits. Live/remix/edit qualifiers,
movement identifiers, and unsupported suffixes remain significant. This is a
recording-title comparison rule, not a change to stored titles or Work identity.
The current one-Work-per-track model cannot represent medleys. MusicBrainz and
previously stored Wikidata IDs remain in `work_external_ids_v1`; newly accepted
recording relationships use `musicbrainz_supported_v1`.

## Storage and API

The additive, repeatable enrichment schema setup creates `works_v1`,
`work_resolutions_v1`, `work_external_ids_v1`, and the discovery cursor in `work_scan_v1`.
`work_resolutions_v1` stores the link or unresolved outcome, reason, timestamps,
and evidence JSON containing prompt version, model/provider, catalog context,
external reference responses with retrieval timestamps, selected source URLs/IDs,
and model responses when used. Work responses expose `wikidata_id` and
`musicbrainz_id`.

Wikidata uses public endpoints and needs no credentials. The implementation uses
its [entity-search API](https://www.mediawiki.org/wiki/Wikibase/API/en) and
[SPARQL service](https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service),
with English labels, up to ten search hits, a 20-second timeout and a 1 MiB
response cap per request. One title is searched per fallback evaluation.
Facts are filtered to musical work/composition (`Q105543609`), composed musical
work (`Q207628`), or song (`Q7366`) classes and their subclasses. A missing creator
label or a truncated fact result is not accepted as a complete identity.

- `GET /v1/content/work/{id}?limit=50&offset=0&scope=all` returns `work`, typed
  `relations`, resolved `tracks`, `next_offset`, and `has_more`. Each track includes
  its album, `recording_work` (ID/title), and `relationship_scope` (`direct`, `part`,
  or `related`). `scope=parts` includes this work and its descendants;
  `scope=related` includes only related works. Invalid scopes return 400.
- `GET /v1/content/works?query=...&limit=25` searches Work titles and creators.
  All query terms must match, in any order, with word-prefix matching and
  case/accent folding. A separate FTS index is backfilled on upgrade and kept
  current as Works change. Web search shows loading, empty, and error states.
- Track detail responses include `work_resolution` and `work_enrichment_status`.
- Web track pages link to `/work/:workId`; Work pages list performances and their
  albums. Web search shows a separate Works section.

Work search results and Work detail headers also expose `creator_artist_ids` and nullable
`composition_year`. Creator portraits resolve imported MusicBrainz creator IDs
against catalog artist IDs, without name matching. The year (or year range) uses
imported composer/writer relationship dates; missing dates stay unset. Recording
release dates and database creation timestamps are never used as composition
dates. Web results use the standard search-row grid, with up to four creator
portraits, an image fallback, and six initial results expandable to the returned
result limit.

Work detail pages display creator portraits, writing dates, ordered parts,
parent links and other directed relationships (arrangements, revisions,
quotations, etc.). Recordings include recursively contained parts, plus works
one non-containment relationship away from the work or its parts and their
contained parts. Related works are labelled separately and filterable; they are
not merged into the work's identity. Traversal deduplicates nodes and tolerates
cycles. It does not walk upwards through parent links or recursively expand an
unbounded chain of related works. Imported writing dates support numeric and
string years; Op. 10's imported `"1829"`–`"1832"` range is displayed correctly.

Android phone support includes clickable resolved Work titles on tracks, a
separate Works section in both catalog and streaming search, and Work pages
with creator portraits, composition dates, directed relationships, scoped
recordings, and cursor-based pagination. Track pages also offer **Play all
versions**, using the same `scope=all` recordings and `work_versions` playback
context as web, including queue persistence and remote playback. Track cache
schema 13 preserves Work resolution; version 12 upgrades without discarding
cached tracks. Work search history, Work likes, persistent offline Work pages,
and dedicated Android TV layouts remain outside this delivery.

Work content routes use the same catalog authentication and rate limits as
tracks. Limits are capped at 100. Deleted catalog tracks are omitted from the
response; pagination offsets count stored links so callers can still advance.

## Evaluating the enrichment model

Use the existing admin job trigger with this parameter to evaluate 1–50 chosen
catalog tracks **without claiming queues, creating Works, or changing links**:

```json
{"work_dry_run_track_ids": ["track-id-1", "track-id-2"]}
```

This invokes the same bounded research loop and intervention model used after
normal retries, retaining results in the job audit without writing Work links or
queue state. One item may take up to 240 seconds; use small batches with the
existing job-runner timeout. The opt-in `work_research_live_corpus` Rust test also
evaluates exported catalog contexts without opening a production database.

Dry runs require an enabled LLM provider and make actual reference/provider requests as needed. The
job audit details contain each proposal, validation failure (if any), model
responses, input context and candidates. This path shares the production
research and validation code. It does not simulate writes between examples: evaluate
matching against pre-existing Works, or use an isolated database to evaluate
sequential creation and linking. Each intervention uses at most five research
rounds and ten new tool calls. The application advances untried searches/fetches;
the model proposes additional actions. Every round receives a fresh prompt from
accumulated source state and previous round outcomes. Repeated actions are skipped,
and format failures do not discard earlier evidence. Deterministic validation
accepts a unique corroborated link without another model decision.

A compact checkpoint carries search outcomes, discovered candidates, full fetched
documents when they fit, and errors into later interventions in the same retry cycle.
Changed catalog identity or manual reopening starts fresh. Oversized documents
are evicted whole and must be fetched again before validation. Source failures
can be retried later; successful empty searches are remembered. Current prompt and
tool traces remain in the evaluation audit, with compact memory in attempt history.

Before accepting a model/prompt for broad use, build a manually reviewed set
containing original/cover/live/remaster groups, different songs with the same
title, multi-writer songs, separate classical movements, arrangements, obscure
tracks, traditional works, medleys and misleading catalog metadata. Include
repeated runs and differently formatted titles. Record expected group identity
or expected abstention before examining model output.

Measure false merges, duplicate Works, missed links, incorrect creations,
abstention rate, and consistency across repeated runs. Review every false merge
and incorrect creation; an aggregate accuracy score can hide these cases. Verify
writer and composition claims against trusted references. Model-reported
confidence is not calibrated evidence of correctness. The legacy proposal score
is now server-assigned for source-supported proposals, not requested from the model.

Automated tests cover normalized reuse, namesakes, movements, weak identity
rejection, retry stability, transactional rollback, schema reopening, separate
queue state, source-only candidate selection, read-only evaluation and
malformed responses. HTTP fixtures cover successful lookup, API errors, rate
limits, and empty results; source validation tests reject fabricated IDs and
contradictory creators. Identity tests cover stable external IDs and namesakes.
An opt-in live smoke test is available as `cargo test --lib wikidata_work_live_lookup -- --ignored --nocapture`.
Catalog-backed tests also exercise discovery eligibility,
scan resumption, retry backoff, cancellation, and linking multiple versions
through the complete identification path. These tests use synthetic identities
and scripted responses; they do not establish a real model's music knowledge.

### Repeatable real-model evaluation

From `pezzottify-server`, run the opt-in harness with the actual enrichment
configuration and a new report path:

```bash
WORK_EVAL_CONFIG=/absolute/path/server.toml WORK_EVAL_REPORT=/tmp/work-eval.jsonl WORK_EVAL_REPEATS=2 cargo test --lib work_evaluation_real_model -- --ignored --nocapture
```

The config must explicitly select a model and enable the agent. The harness uses
the production provider factory, prompts, Wikidata lookup and Work storage, but
only opens temporary databases; it does not open the production catalog or queue.
Provider calls may incur the usual costs. Reports include model responses and
source evidence; treat their contents as potentially sensitive.

`tests/fixtures/work-evaluation-v1.json` contains 16 synthetic catalog contexts
with separately reviewed expected identities and source links. It covers shared
versions, covers, namesakes, movement granularity, ambiguity, invented titles,
medleys, spoken content and instruction-like catalog text. Expected labels are
not passed to the model. The second repetition reverses processing order.

JSONL records preserve each result and flag false creations, missed identities,
wrong creators/titles/kinds/external IDs, duplicate Works, false merges, repeat
inconsistency and pipeline failures. A missing Wikidata ID is allowed when the
inferred identity is otherwise correct; a passing score does not imply full
external-source coverage. Any failed case fails the test after writing its report.
This is an initial regression corpus, not a comprehensive acceptance evaluation:
extend it with reviewed real-catalog cases, arrangements and traditional works
before broad rollout. Compiling or running the scoring unit tests is not evidence
that the configured model passed this evaluation.

### Thinking-model deployment requirements

Artist, album, track and Work enrichment support per-request reasoning controls
under `[agent.llm]`, using SimpleAI's existing API. Both are omitted by default
for compatibility. An experimental bounded-thinking configuration is:

```toml
[agent.llm]
provider = "openai"
model = "class:fast"
reasoning_effort = "low"
thinking_budget_tokens = 512
```

These controls require advertised support on every eligible runner; the currently
configured Qwen capabilities only allow `none`, not `low` or a numeric budget.
The direct Qwen runtime probe with `low` and budget 512 returned complete JSON in
11.5 seconds (649 generated tokens); budget 64 returned JSON in 5.3 seconds (271
tokens), with the same total limit of 1,500. These were direct runtime checks,
not a verified `class:fast` deployment configuration.

No SimpleAI response-behavior change is required: its existing implementation
prefers final content whenever present. The proposed shared response change was
reverted. Keep adaptation in Pezzottify's requests and validation. Enrichment
rejects truncated, empty and non-successfully-finished answers before parsing or
storage; partial JSON is not accepted just because it happens to parse.

On 2026-09-13, the baseline synthetic Yesterday request exhausted 1,500 tokens
and returned non-JSON reasoning. The same request with thinking disabled returned
complete JSON in 9.6 seconds directly on halo1 and 9.7 seconds through SimpleAI.
These are response-format checks, **not identity-quality passes**: the returned
creator credits still failed the reviewed expectation. The full 16-case follow-up
was blocked before inference by Wikidata `maxlag` responses. Do not bypass source
backoff or treat that run as a model-quality result.

Halo1 and halo2 received the opt-in config on that date, with backups alongside
`/home/lelloman/config.toml` named `config.toml.before-work-reasoning-*`.
Those earlier Halo capability changes remain opt-in and do not alter defaults.
RTX was offline and its live configuration remains unverified. Pezzottify's
binary and production reasoning settings still require rollout;
temporary evaluation settings do not change production enrichment.

## Deliberate v1 limits

Each track links to at most one Work; composite tracks abstain. Work aliases
and manual merge/correction tools are not implemented.
Imported parent/child and other Work relationships are browsable on the web.
Existing links are immutable to
automatic enrichment; a future correction flow must preserve attachment
identity. Existing linked tracks are not automatically revalidated; dry runs can
evaluate them against references. Missing source coverage yields abstention,
not an invented composition. Lyrics and sheet music can later attach
to the stable Work UUID; this change does not implement attachments.
