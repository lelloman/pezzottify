# Steering concepts and destination mixes

Smart continuation can steer a queue from its Source toward a Destination (see
`radio-playback.md`). A Destination is a weighted mix of up to 8 components. Each
component is a catalog item (track, album, artist) or a **concept**: a named musical
idea with a vector in every track embedding namespace.

## Concepts

A concept's vector is the mean of the unit-length vectors of its example tracks, with
at most 5 example tracks per artist so one artist cannot define a concept. Concepts with
fewer than 30 qualifying examples are not created. A weekly background job
(`concept_embedding_sync`, also triggerable through the admin jobs API) recomputes them
and removes concepts that no longer qualify.

| Family | Id format | Label example | Examples |
|---|---|---|---|
| `sound_genre` | `audioset:Jazz` | Jazz | Top 200 tracks by `ast.audioset.v2` score, score >= 0.15 |
| `instrument` | `audioset:Piano` | Piano | same |
| `vocals` | `audioset:Choir` | Choir | same |
| `mood` | `audioset:Sad music` | Sad music | same |
| `genre_tag` | `genre:psytrance` | psytrance | Tracks of artists tagged with it; tags with >= 50 available tracks; up to 200 sampled deterministically |
| `recorded` | `recorded:1960s` | Recorded in the 1960s | Tracks whose recording (earliest release among tracks sharing the ISRC) falls in the decade |
| `composed` | `composed:1810s` | Composed in the 1810s | Tracks linked to a Work whose composition year falls in the decade; a range is spread over the decades it spans |

Concept vectors are stored as `entity_embeddings` rows with `entity_type = "concept"`.

### `GET /v1/content/concepts`

Query parameters: `q` (case-insensitive substring on the label, optional), `family`
(optional), `limit` (default 50, max 500). Requires catalog access. Response:

```json
{ "concepts": [
  { "id": "audioset:Jazz", "family": "sound_genre", "label": "Jazz", "example_count": 200 }
] }
```

Ordered by family, then label. Decade families are ordered chronologically.

## References

Every reference (`source_references`, `destination`, `away`) accepts
`entity_type` = `track | album | artist | concept`, `entity_id`, optional positive `weight`.

## Continuation request and response changes

- `destination` accepts either one reference object (legacy) or an array of up to 8.
  The destination vector is the normalised weighted sum of the components' unit vectors,
  per namespace. Components without a vector in a namespace are skipped there.
- Each `namespaces[]` diagnostics entry gains `destination_components`:
  `[{ "entity_type", "entity_id", "similarity" }]`, the cosine similarity between the
  current query and that component (`null` when the component has no vector there).
  `query_to_destination` keeps describing the whole mix.

## Gravity state v2

```jsonc
{
  "v": 2,
  "source": { "kind": "queue" },                 // or {"kind":"references","references":[...]} (max 8)
  "auto_track_ids": ["..."],
  "destination": null,                           // or [ {entity_type, entity_id, label, weight} ] (1..8)
  "steps_total": 20,
  "steps_done": 0,
  "knobs": { "recency_weight": null, "criteria": null, "diversity": null, "randomness": null, "mode": null, "away": [] },
  "last_diagnostics": null
}
```

- A single destination is an array of one. On arrival the whole array becomes the Source
  (`kind: "references"`).
- v1 gravity objects (destination as a single object) are not migrated: a non-array
  destination is read as `null`.
- The continuation request sends `destination` as the array (without labels) and
  `progress = steps_done / steps_total`.
- `last_diagnostics.namespaces[].destination_components` drives per-component progress
  in the steering UI.
