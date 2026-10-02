# Radio playback and artist greatest hits

Radio queues own their continuation; the Smart continuation preference applies only to ordinary queues. Track, album, artist, genre, and custom radios retain their original seed or filters and request 10 more tracks when zero or one remain. Generated and manually added track IDs stay excluded even after played queue history is trimmed. An empty successful response exhausts the generator; request failures allow bounded retries and an explicit retry action.

Artist **Play greatest hits** is a radio with `source: "greatest_hits"`. The server returns all currently available credited tracks in popularity-descending, ID-ascending order, deduplicating nonempty ISRCs. The highest-ranked available release represents each ISRC; tracks without an ISRC are distinct. Clients retain that snapshot and append 10 IDs at a time without downloading the entire artist's metadata or audio. Exhaustion stops playback without switching to related music.

## API

- `GET /v1/content/artist/{id}/greatest-hits` returns `{ "track_ids": [...] }`. A known artist with no available tracks returns an empty list; an unknown artist returns 404.
- `POST /v1/content/radio/continue` accepts `source` (`basic`, `custom`, or `genre`), `seed` (`entity_type`, `entity_id`), original `settings`, `context_track_ids`, `exclude_track_ids`, and `count` (default/max 10). It returns `{ "track_ids": [...] }`. Custom settings use the existing radio-build recipe schema. Greatest hits consumes its snapshot locally and does not call this endpoint.

Both routes use existing catalog authorization and content rate limits. Existing radio creation routes remain compatible.

## State and settings

Playback playlists carry origin metadata and a separate continuation value. Queue synchronization and remote `loadTrackIds` commands place this value under `context.continuation`:

```json
{
  "session_id": "unique-session-id",
  "strategy": "ranked_snapshot",
  "status": "active",
  "seen_track_ids": ["first"],
  "ordered_track_ids": ["first", "second"],
  "next_index": 1
}
```

Other radios use `strategy: "seeded_radio"`. Status is `active`, `stopped`, or `exhausted`. Persist this state with the queue, but omit it from listening-event metadata. Only the playing device generates batches. Keep at most 500 loaded entries by trimming preceding history when needed; retain exclusion history separately.

The synchronized boolean preference `keep_radio_on_queue_edit` defaults to `true`. It is presented as **When editing a radio queue → Keep radio going / Stop adding tracks**. Successful manual additions, removals, and reorders mark the radio edited. With the preference disabled, an edit stops generation for that session; it does not convert the queue into a smart-continuation mix. Preference changes alone do not restart or stop sessions. Automatic appends and history trimming are not edits. Saving a queue saves its loaded tracks as an ordinary playlist.

Deploy the server additions before updated clients. New behavior requires updated clients; no database schema migration is needed.

## Smart continuation and gravity

Smart continuation extends ordinary (non-radio) queues. Its query is anchored on the tracks the user chose, not on the tail of the queue. Earlier versions averaged the last 10 queue tracks, including tracks the server had appended, so the queue drifted away from the user's choice.

### `POST /v1/content/recommendations/continuation`

| Field | Meaning |
|---|---|
| `source_track_ids` | User-chosen queue tracks, at most 200. The server samples 64 evenly and deterministically. |
| `source_references` | Alternative or additional source: up to 8 `{entity_type, entity_id, weight?}` for `track`, `album`, `artist`. |
| `recent_track_ids` | Last few played tracks of any provenance. The server keeps the last 10. |
| `recency_weight` | Share of the recent-tracks vector in the query, default 0.2, clamped to 0..1. |
| `destination` | Optional reference to steer toward. |
| `progress` | 0..1 position between source and destination, default 0. |
| `criteria`, `diversity`, `randomness`, `mode`, `away` | Same meaning as the radio builder. One namespace by default, diversity and randomness 0.3, mode `similar`. |
| `exclude_track_ids`, `count` | Hard exclusions and number of tracks, 1..10. |
| `context_track_ids` | Legacy. Used only when no source or destination is sent. |

The query is built per namespace from unit-length vectors: the source vector, interpolated toward the destination by `progress`, blended with the recent-tracks vector by `recency_weight`, then pushed away from `away` references. Candidates go through the radio scoring and diversity selection, primed with the recent tracks so recently heard artists are deferred.

The response is `{ "track_ids", "recency_weight", "progress", "namespaces" }`. Each namespace entry reports `source_to_destination`, `query_to_source`, and `query_to_destination` cosine similarities, or `null` when a vector is missing. Invalid input returns 400 with code `invalid_continuation_request`. `mode: "explore"` deliberately avoids the closest matches, so it never "arrives"; clients should not offer it while a destination is set.

### Gravity state

Clients keep a `gravity` object next to `continuation` on each non-radio playlist, persist it with the queue, and synchronize it under `context.gravity`. Remote controllers change it with the `setGravity` command.

```json
{
  "v": 1,
  "source": { "kind": "queue" },
  "auto_track_ids": ["appended-by-smart-continuation"],
  "destination": { "entity_type": "artist", "entity_id": "id", "label": "Name" },
  "steps_total": 20,
  "steps_done": 4,
  "knobs": { "recency_weight": null, "criteria": null, "diversity": null, "randomness": null, "mode": null, "away": [] },
  "last_diagnostics": null
}
```

With `source.kind: "queue"`, the source is every queue track not in `auto_track_ids`, so tracks the user adds later join it. A user re-adding a suggested track promotes it to the source. Removed suggestions stay excluded. Each appended track advances `steps_done`; `progress` is `steps_done / steps_total`. On arrival the destination becomes the source (`kind: "references"`), the destination is cleared and `steps_done` resets. Starting a new playlist resets gravity. Knobs set to `null` use server defaults.
