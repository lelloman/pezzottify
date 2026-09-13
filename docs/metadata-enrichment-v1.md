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
candidates or truncated search results do not resolve an identity.

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

Unresolved/ambiguous identity, HTTP errors, rate limits and Wikidata API errors
remain retryable with the configured backoff. They never trigger unsourced
generation. Missing catalog items are permanent failures for that attempt.
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
