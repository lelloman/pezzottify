# Works v1

A Work identifies a composition or other single performable unit, independently
of its recordings. Tracks with the same canonical title, creator set and catalog
number share a stable UUID. An individual movement or aria has its own Work;
its title must identify its parent composition and movement. Work identity does
not include the performer, album, live/remaster suffix, or ISRC.

## Queue and resolution

The `metadata_enrichment_v1` background job now accepts `work_resolution` as an
entity type. Its queue entity ID is a **track ID**, not a Work ID. Work resolution
is independent of ordinary track metadata enrichment.

The initial interpretation of “known track” is a catalog track with local audio
available (`track_available = 1` and a non-null audio URI). When the enrichment
queue is idle, discovery admits up to 100 such tracks per run, scans at most
5,000 rows, and persists its offset. At the end of the catalog it wraps around,
so newly available tracks are eventually picked up. Existing queued, running,
failed, or completed resolutions are not automatically reset. Normal listening
metadata keeps its higher priority. A manual run can isolate resolution:

```json
{"entity_types": ["work_resolution"], "batch_size": 25}
```

Resolution uses Wikidata and the existing configured LLM provider, with temperature zero:

1. Search Wikidata using the existing work title or catalog track title. Fetch
   composition/song candidates and their composer/lyricist claims, excluding
   albums and audio tracks. Give those references and the catalog context to the
   model. Return an explicit unresolved result when unsure.
2. Search local Works using the proposed canonical title (up to 25 candidates).
   If the first Wikidata lookup found no usable candidates and the model proposes
   a different canonical title, retry external search once with that title.
   If local or external candidates exist, ask the model to reconsider using them.
3. Validate the proposal on the server. Require a nonempty title, named creators,
   an explanation, an allowed work kind, and confidence of at least 0.9.
   A cited Wikidata ID must have been fetched, and the proposed title and complete
   creator set must agree with that reference. Fabricated or contradictory
   citations are retryable model errors. Matching a track to that composition
   remains an inference; the reference supports its identity facts.
4. In an immediate transaction, reuse the exact normalized identity or create a
   new Work, then persist the track link and evidence together. A unique identity
   key prevents duplicate creation for the same normalized identity.
   Validated Wikidata IDs are stored separately and take precedence over text
   matching. Different Wikidata IDs do not merge on identical titles/creators.

Normalization folds case and whitespace and sorts/deduplicates creators; it
preserves punctuation, accents, catalog suffixes, and movement identifiers.
`song` versus `standard` is descriptive and does not split an identity. Unknown
creators, medleys, mashups, and ambiguous identities are unresolved in v1.
Wikidata network errors, HTTP 429, and API errors such as `maxlag` retry rather
than falling back to unsupported guesses. A successful lookup with no matching
reference can still produce an explicitly inferred Work (`llm_work_v2`). A link
with a validated reference is marked `wikidata_supported_v1`. Both external and
model network/malformed-output failures use the existing queue retry/backoff and
cancellation recovery. A retry never replaces an already linked Work.

## Storage and API

The additive, repeatable enrichment schema setup creates `works_v1`,
`work_resolutions_v1`, `work_external_ids_v1`, and the discovery cursor in `work_scan_v1`.
`work_resolutions_v1` stores the link or unresolved outcome, reason, timestamps,
and evidence JSON containing prompt version, model/provider, catalog context,
local candidates, external search/fact responses with retrieval timestamps,
selected source URLs/Q-IDs, and model responses. Work responses expose
`wikidata_id`, and web Work pages link to the reference.

Wikidata uses public endpoints and needs no credentials. The implementation uses
its [entity-search API](https://www.mediawiki.org/wiki/Wikibase/API/en) and
[SPARQL service](https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service),
with English labels, up to ten search hits, a 20-second timeout and a 1 MiB
response cap per request. At most two titles are searched per evaluation.
Facts are filtered to musical work/composition (`Q105543609`), composed musical
work (`Q207628`), or song (`Q7366`) classes and their subclasses. A missing creator
label or a truncated fact result is not accepted as a complete identity.

- `GET /v1/content/work/{id}?limit=50&offset=0` returns `work`, resolved `tracks`,
  `next_offset`, and `has_more`. Each resolved track also includes its album.
- `GET /v1/content/works?query=...&limit=25` searches Work titles. This is a bounded
  title substring search, independent of the main catalog search index.
- Track detail responses include `work_resolution` and `work_enrichment_status`.
- Web track pages link to `/work/:workId`; Work pages list performances and their
  albums. Web search shows a separate Works section.

Work content routes use the same catalog authentication and rate limits as
tracks. Limits are capped at 100. Deleted catalog tracks are omitted from the
response; pagination offsets count stored links so callers can still advance.

## Evaluating the enrichment model

Use the existing admin job trigger with this parameter to evaluate 1–50 chosen
catalog tracks **without claiming queues, creating Works, or changing links**:

```json
{"work_dry_run_track_ids": ["track-id-1", "track-id-2"]}
```

Dry runs require an enabled LLM provider and make actual Wikidata and provider requests. The
job audit details contain each proposal, validation failure (if any), model
responses, input context and candidates. This path shares the production
identification code. It does not simulate writes between examples: evaluate
matching against pre-existing Works, or use an isolated database to evaluate
sequential creation and linking. Each track takes one model call, or two when
local or external candidates are available, plus up to four Wikidata requests.

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
confidence is only an admission heuristic and is not calibrated evidence of
correctness. The default 0.9 threshold needs evaluation with the chosen model.

Automated tests cover normalized reuse, namesakes, movements, weak identity
rejection, retry stability, transactional rollback, schema reopening, separate
queue state, bounded candidate reconsideration, read-only evaluation and
malformed responses. HTTP fixtures cover successful lookup, API errors, rate
limits, and empty results; source validation tests reject fabricated IDs and
contradictory creators. Identity tests cover stable external IDs and namesakes.
An opt-in live smoke test is available as `cargo test --lib wikidata_work_live_lookup -- --ignored --nocapture`.
Catalog-backed tests also exercise discovery eligibility,
scan resumption, retry backoff, cancellation, and linking multiple versions
through the complete identification path. These tests use synthetic identities
and scripted responses; they do not establish a real model's music knowledge.

## Deliberate v1 limits

Each track links to at most one Work; composite tracks abstain. Parent/child
Work relationships, aliases, non-Wikidata external Work IDs, manual merge/correction tools,
and Android Work screens are not implemented. Existing links are immutable to
automatic enrichment; a future correction flow must preserve attachment
identity. Existing linked tracks are not automatically revalidated; dry runs can
evaluate them against Wikidata. Conservative text matching without a selected
external reference can create separate Works when spellings,
creator sets or catalog numbers differ. Lyrics and sheet music can later attach
to the stable Work UUID; this change does not implement attachments.
