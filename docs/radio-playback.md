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
