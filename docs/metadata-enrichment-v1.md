# Metadata Enrichment v1

Metadata Enrichment v1 stores generated artist, album, and track facts in explicit SQLite tables instead of generic JSON profiles. The canonical rows live in `enrichment.db` and are designed for filtering, joins, and detail-page status display.

## Storage Model

Primary typed tables:

| Table | Entity | Purpose |
| ----- | ------ | ------- |
| `artist_enrichment_v1` | artist | Person/group role flags, dates, origin, language, confidence, summary, bio |
| `album_enrichment_v1` | album | Release/recording dates, country, label/catalog data, album flags, summary, notes |
| `track_enrichment_v1` | track | Work/performance fields, classical metadata, language, track flags, summary, notes |

Shared child tables:

| Table | Purpose |
| ----- | ------- |
| `enrichment_queue_v1` | Queue state for missing or stale entity enrichment |
| `entity_tags_v1` | Queryable genre/style/mood/scene/theme tags |
| `entity_contributors_v1` | Contributor names, optional local IDs, roles, confidence |
| `entity_relations_v1` | Local or external relations with confidence and visibility gating |
| `entity_aliases_v1` | Alternate names with locale/source/confidence |
| `entity_external_ids_v1` | Provider IDs and URLs such as ISRC or UPC |
| `entity_sources_v1` | Source names, URLs, retrieval time, confidence |
| `entity_evidence_v1` | Supplemental snippets or raw JSON payloads, not the query surface |

Legacy `artist_enrichment` and `album_enrichment` remain for compatibility. New generated metadata should use the `_v1` tables.

## Queue Behavior

The server enqueues work without blocking user requests:

- `POST /v1/user/impression` enqueues the viewed artist, album, or track with reason `impression` and priority `5`.
- `POST /v1/user/listening` enqueues a completed track play with reason `listening` and priority `20`.
- Completed listening events also enqueue the track's album and artists with reason `listening_adjacent` and priority `10`.
- Enqueue is deduplicated per entity and only happens when the v1 row is missing or older than the stale threshold, currently 90 days.

Detail content responses include `enrichment_status` when the enrichment database has queue or completed state for the entity. The status reports `status`, `stage`, `attempts`, `last_error`, timestamps, and completed enrichment metadata.

## Background Job

The `metadata_enrichment_v1` background job runs on the configured interval and can also be triggered from the admin jobs panel.

Default configuration:

```toml
[background_jobs.metadata_enrichment]
interval_hours = 6
batch_size = 25
retry_after_secs = 21600
```

Manual trigger params:

```json
{
  "batch_size": 10,
  "entity_types": ["artist", "album", "track"]
}
```

`entity_types` is optional. When omitted, the job claims all supported entity types. These trigger parameters only scope a single run; they do not change the shared LLM provider settings.

## LLM Configuration

The job reuses the existing shared `[agent]` / `[agent.llm]` configuration. It does not add metadata-specific model, provider, base URL, or API key settings. The existing agent LLM supports Ollama and OpenAI-compatible providers. Simple-AI can be used if it exposes an OpenAI-compatible chat endpoint.

```toml
[agent]
enabled = true

[agent.llm]
provider = "openai"
base_url = "http://simple-ai:8000/v1"
model = "your-chat-model"
temperature = 0.2
timeout_secs = 120
```

If `agent.enabled` is false, the job marks claimed rows as queued again with `last_error = "agent LLM is disabled"` and `next_attempt_at` set from `retry_after_secs`.

## Generated Output Contract

The job prompts the LLM for strict JSON and parses the first complete JSON object in the response. The generated data is normalized before storage:

- Empty strings and literal `"null"` are discarded.
- Confidence values are clamped to `0.0..=1.0`.
- Album UPC and track ISRC values from the catalog are preserved as external IDs.
- Relations are visible only when confidence is at least `0.8`; lower-confidence relations remain stored but hidden from visible relation queries.
- Raw LLM JSON is stored as evidence for audit/debugging, not as the query surface.

## Operational Notes

- The job is retry-oriented. LLM errors, malformed JSON, and transient database write failures leave the queue item retryable.
- Missing catalog entities are permanent failures because there is no source context to enrich.
- Generated facts should be treated as inferred metadata unless source-backed enrichment is added later.
- The admin panel can limit a manual run by batch size and entity type, which is useful when validating a new model or provider configuration.
