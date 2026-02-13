# Related Artists via Last.fm + MusicBrainz

**Status**: ✅ **IMPLEMENTED**

## Problem

Spotify's Web API (`/artists/{id}/related-artists`) is aggressively rate-limited (429), making it unusable for fetching similar artists through the downloader. The librespot Mercury API has a `related` field on artist metadata but it always returns empty.

## Solution

Use Last.fm's `artist.getSimilar` API for similarity data, with MusicBrainz as the ID bridge between Spotify and Last.fm.

### ID Mapping Chain

All lookups are ID-based — no name matching needed.

```
Spotify ID ──► MusicBrainz (mbid) ──► Last.fm (similar artists + mbids) ──► MusicBrainz ──► Spotify IDs
```

#### Step 1: Spotify ID → MusicBrainz mbid

```
GET https://musicbrainz.org/ws/2/url?resource=https://open.spotify.com/artist/{spotify_id}&inc=artist-rels&fmt=json
```

Response contains `relations[].artist.id` → the mbid.

Example: `4Z8W4fKeB5YxbusRsdQVPb` (Radiohead) → `a74b1b7f-71a5-4011-9441-d0b5e4122711`

#### Step 2: mbid → Last.fm Similar Artists

```
GET https://ws.audioscrobbler.com/2.0/?method=artist.getsimilar&mbid={mbid}&api_key={key}&format=json&limit=10
```

Returns similar artists with match scores (0-1) and their MusicBrainz IDs.

Example result for Radiohead:
| Artist | Match | mbid |
|--------|-------|------|
| Thom Yorke | 1.0 | 8ed2e0b3-... |
| Atoms for Peace | 0.59 | 9e299bee-... |
| Jeff Buckley | 0.42 | e6e879c0-... |
| The Strokes | 0.33 | f181961b-... |
| Muse | 0.32 | 9c9f1380-... |

#### Step 3: Similar artist mbid → Spotify ID

```
GET https://musicbrainz.org/ws/2/artist/{mbid}?inc=url-rels&fmt=json
```

Find `open.spotify.com/artist/` in `relations[].url.resource`.

Example: `8ed2e0b3-...` (Thom Yorke) → `4CvTDPKA6W06DRfBnZKrau`

### Caching Strategy

Store `mbid` as a field on the artist in the catalog database. This makes the Spotify↔MusicBrainz mapping permanent — looked up once per artist, never again.

With a warm cache:
- Step 1: catalog lookup (free)
- Step 2: 1 Last.fm API call
- Step 3: catalog lookup for known artists, MusicBrainz call only for new ones

## API Details

### Last.fm
- **API key**: Set via `lastfm_api_key` in config TOML
- **Rate limit**: 5 requests/second
- **Docs**: https://www.last.fm/api/show/artist.getSimilar

### MusicBrainz
- **No API key needed** (just a User-Agent with contact info)
- **Rate limit**: ~1200 requests per window (generous)
- **Docs**: https://musicbrainz.org/doc/MusicBrainz_API

## Implementation Notes

- Both APIs are free and have stable, generous rate limits
- MusicBrainz requires a descriptive `User-Agent` header (e.g., `pezzottify/1.0 (contact@example.com)`)
- Some artists may lack an mbid in Last.fm or a Spotify link in MusicBrainz — skip these gracefully
- Related artists data is stable over time, so results can be cached for days/weeks

## Implementation Status

Implemented as a background job (`RelatedArtistsEnrichmentJob`) with two-phase execution:

- **Phase 1**: Batch of artists with `mbid_lookup_status=0` → MusicBrainz lookup → status 1 (found) or 2 (not found)
- **Phase 2**: Batch of artists with `mbid_lookup_status=1` → Last.fm similar → resolve mbids back to catalog → store relationships → status 3 (done)

### Schema (migration v5)

- `artists` table: added `mbid TEXT`, `mbid_lookup_status INTEGER NOT NULL DEFAULT 0`
- New `related_artists` table: `artist_rowid`, `related_artist_rowid`, `match_score REAL`

### Configuration

Enabled via `[related_artists]` section in config TOML. Requires `lastfm_api_key` and `musicbrainz_user_agent`. See `config.example.toml`.

### Key files

- `src/related_artists/musicbrainz.rs` — MusicBrainz API client (rate limited 1 req/sec)
- `src/related_artists/lastfm.rs` — Last.fm API client (rate limited 5 req/sec)
- `src/background_jobs/jobs/related_artists_enrichment.rs` — the background job
- `src/catalog_store/schema.rs` — migration v5
