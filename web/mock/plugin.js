import { fileURLToPath } from "node:url";
import { WebSocketServer } from "ws";
import {
  artists,
  albums,
  tracks,
  genres,
  work,
  permissions,
  screens,
  freshState,
  resolvedTrack,
  resolvedAlbum,
  artwork,
  audio,
} from "./fixtures.js";

export function mockPlugin() {
  let state = freshState();
  let deviceSharePolicy = {
    mode: "deny_everyone",
    allow_users: [],
    deny_users: [],
    allow_roles: [],
  };
  const misses = new Set();
  const sound = audio();
  const requests = [
    {
      id: "request-1",
      content_type: "ALBUM",
      content_id: "album-12",
      content_name: "A Little Further",
      artist_name: "Luca Moretti",
      status: "PENDING",
      priority: "USER",
      created_at: 1780000000,
      queue_position: 1,
    },
    {
      id: "request-2",
      content_type: "ALBUM",
      content_id: "album-3",
      content_name: "Tidal Patterns",
      artist_name: "Juniper Coast",
      status: "COMPLETED",
      priority: "USER",
      created_at: 1780000000,
      completed_at: 1780000300,
    },
  ];
  requests.push(
    ...[
      ["album-9", "IN_PROGRESS"],
      ["album-10", "FAILED"],
      ["album-11", "COMPLETED"],
      ["track-68", "PENDING"],
      ["track-69", "IN_PROGRESS"],
      ["track-70", "COMPLETED"],
      ["track-71", "FAILED"],
    ].map(([id, status]) => ({
      id: `request-${id}`,
      content_id: id,
      content_type: id.startsWith("album") ? "ALBUM" : "TRACK",
      content_name: "Preview download",
      status,
      priority: "USER",
      created_at: 1780000000,
      queue_position: status === "PENDING" ? 2 : null,
      progress:
        status === "IN_PROGRESS" ? { completed: 3, total_children: 6 } : null,
    })),
  );
  const initialRequests = structuredClone(requests);
  const ingestionJobs = [
    {
      id: "ingestion-1",
      original_filename: "Golden Hour.flac",
      file_count: 1,
      total_size_bytes: 10485760,
      status: "COMPLETED",
      upload_type: "FILE",
      detected_artist: "Mira Sol",
      detected_album: "Golden Hour",
      created_at: 1780000000,
    },
  ];
  const json = (res, value, status = 200) => {
    res.writeHead(status, {
      "Content-Type": "application/json",
      "Cache-Control": "no-store",
    });
    res.end(JSON.stringify(value));
  };
  const list = (items) => (state.scenario === "empty" ? [] : items);
  const search = (q) =>
    list(
      [
        ...artists.map((a) => ({ ...a, type: "Artist" })),
        ...albums.map((a) => ({ ...a, type: "Album" })),
        ...tracks.map((t) => ({
          ...t,
          duration: t.duration_ms / 1000,
          type: "Track",
        })),
      ].filter((x) =>
        `${x.name} ${x.artists_ids_names?.flat().join(" ") || ""}`
          .toLowerCase()
          .includes(q.toLowerCase()),
      ),
    );
  const paged = (items, key = "items") => ({
    [key]: list(items),
    total: list(items).length,
    has_more: false,
    next_offset: items.length,
  });
  const panel = `<!doctype html><meta name="viewport" content="width=device-width"><title>Pezzottify design lab</title><style>body{background:#121212;color:#eee;font:16px system-ui;max-width:1000px;margin:40px auto;padding:24px}a,button,select{color:#eee;background:#252525;border:1px solid #444;border-radius:8px;padding:12px;text-decoration:none}nav{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px}h1{color:#1ed760}button{cursor:pointer}form{display:flex;gap:12px;margin:24px 0;flex-wrap:wrap}</style><h1>Pezzottify design lab</h1><p>Real screens, fictional data. Changes stay in this local server's memory. Restart or reset to restore fixtures.</p><form><label>Scenario <select id="scenario">${["populated", "empty", "slow", "error", "signed-out"].map((x) => `<option>${x}</option>`).join("")}</select></label><button>Apply and open app</button><button type="button" id="reset">Reset fixtures</button></form><p>Playback uses a quiet 90-second test tone. A paused queue is preloaded; press Play to test transport and synced lyrics. Login accepts any non-empty username/password.</p><h2>Download states</h2><nav><a href="/album/album-8">Album · Request download</a><a href="/album/album-12">Album · Requested</a><a href="/album/album-9">Album · Downloading</a><a href="/album/album-11">Album · Available (no download control)</a><a href="/album/album-10">Album · Failed / retry</a><a href="/track/track-67">Track · Request download</a><a href="/track/track-69">Track · Downloading</a><a href="/track/track-71">Track · Failed / retry</a></nav><h2>All screens</h2><nav>${screens.map(([title, path]) => `<a href="${path}">${title}</a>`).join("")}</nav><p><a href="/__mock/status">Mock status / unhandled requests</a></p><script>const apply=async(s)=>{await fetch('/__mock/scenario',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({scenario:s})});localStorage.clear();sessionStorage.clear();location.href=s==='signed-out'?'/login':'/'};document.querySelector('form').onsubmit=e=>{e.preventDefault();apply(document.querySelector('select').value)};document.querySelector('a[href="/login"]').onclick=e=>{e.preventDefault();apply('signed-out')};document.querySelector('#reset').onclick=()=>apply('populated');fetch('/__mock/status').then(r=>r.json()).then(s=>document.querySelector('select').value=s.scenario);</script>`;
  async function handle(req, res, next) {
    const url = new URL(req.url, "http://localhost");
    const path = decodeURIComponent(url.pathname);
    const method = req.method;
    if (!path.startsWith("/v1/") && !path.startsWith("/__mock")) return next();
    let body = {};
    if (["POST", "PUT", "PATCH"].includes(method)) {
      let raw = "";
      for await (const part of req) {
        raw += part;
        if (raw.length > 1_000_000)
          return json(res, { error: "Mock upload limit: 1 MB" }, 413);
      }
      try {
        body = raw ? JSON.parse(raw) : {};
      } catch {
        return json(
          res,
          { error: "Mock expects JSON; file ingestion is not simulated" },
          415,
        );
      }
    }
    if (path === "/__mock") {
      res.writeHead(200, { "Content-Type": "text/html" });
      return res.end(panel);
    }
    if (path === "/__mock/status")
      return json(res, {
        scenario: state.scenario,
        unhandled: [...misses],
        screens,
      });
    if (path === "/__mock/downloads/reset" && method === "POST") {
      requests.splice(0, requests.length, ...structuredClone(initialRequests));
      return json(res, { ok: true });
    }
    if (path === "/__mock/scenario" && method === "POST") {
      if (
        !["populated", "empty", "slow", "error", "signed-out"].includes(
          body.scenario,
        )
      )
        return json(res, { error: "Unknown scenario" }, 400);
      state = freshState();
      requests.splice(0, requests.length, ...structuredClone(initialRequests));
      state.scenario = body.scenario;
      state.loggedIn = body.scenario !== "signed-out";
      misses.clear();
      return json(res, { ok: true });
    }
    if (path === "/v1/auth/login") {
      state.loggedIn = true;
      return json(res, { success: true });
    }
    if (path === "/v1/auth/logout") {
      state.loggedIn = false;
      return json(res, { success: true });
    }
    if (path === "/v1/auth/session")
      return json(
        res,
        state.loggedIn
          ? { user_handle: "design-demo", permissions }
          : { error: "Sign in to the local demo" },
        state.loggedIn ? 200 : 401,
      );
    if (!state.loggedIn)
      return json(res, { error: "Local session signed out" }, 401);
    if (state.scenario === "slow")
      await new Promise((r) => setTimeout(r, 1800));
    if (state.scenario === "error" && !path.startsWith("/v1/sync/"))
      return json(res, { error: "Simulated design-lab failure" }, 503);
    if (path.startsWith("/v1/content/image/")) {
      res.writeHead(200, {
        "Content-Type": "image/svg+xml",
        "Cache-Control": "no-store",
      });
      return res.end(artwork(path.split("/").at(-1)));
    }
    if (path.startsWith("/v1/content/stream/")) {
      const range = /bytes=(\d+)-(\d*)/.exec(req.headers.range || "");
      const start = range ? Number(range[1]) : 0,
        end =
          range && range[2]
            ? Math.min(Number(range[2]), sound.length - 1)
            : sound.length - 1;
      if (start > end || start >= sound.length)
        return res
          .writeHead(416, { "Content-Range": `bytes */${sound.length}` })
          .end();
      res.writeHead(range ? 206 : 200, {
        "Content-Type": "audio/wav",
        "Accept-Ranges": "bytes",
        "Content-Length": end - start + 1,
        ...(range
          ? { "Content-Range": `bytes ${start}-${end}/${sound.length}` }
          : {}),
      });
      return res.end(sound.subarray(start, end + 1));
    }
    if (path === "/v1/sync/state")
      return json(res, {
        ...state,
        likes:
          state.scenario === "empty"
            ? { albums: [], artists: [], tracks: [] }
            : state.likes,
        playlists: list(state.playlists),
      });
    if (path === "/v1/sync/events")
      return json(res, { events: [], has_more: false, latest_seq: state.seq });
    if (path === "/v1/user/settings") {
      if (method === "PUT") state.settings = body.settings;
      return json(res, state.settings);
    }
    const like = path.match(
      /^\/v1\/user\/liked\/(album|artist|track)(?:\/([^/]+))?$/,
    );
    if (like) {
      const key = like[1] + "s";
      if (like[2]) {
        state.likes[key] = state.likes[key].filter((x) => x !== like[2]);
        if (method === "POST") state.likes[key].push(like[2]);
      }
      return json(res, list(state.likes[key]));
    }
    if (path === "/v1/user/playlists") return json(res, list(state.playlists));
    if (path === "/v1/user/playlist" && method === "POST") {
      const p = {
        id: `playlist-${state.playlists.length + 1}`,
        name: body.name || "New playlist",
        tracks: [],
      };
      state.playlists.push(p);
      return json(res, p.id);
    }
    const pl = path.match(/^\/v1\/user\/playlist\/([^/]+)(?:\/(add|remove))?$/);
    if (pl) {
      const p = state.playlists.find((x) => x.id === pl[1]);
      if (!p) return json(res, { error: "Unknown playlist" }, 404);
      if (method === "DELETE")
        state.playlists = state.playlists.filter((x) => x !== p);
      if (method === "PUT") {
        if (pl[2] === "add")
          p.tracks.push(...(body.tracks_ids || body.track_ids || []));
        else if (pl[2] === "remove")
          p.tracks = p.tracks.filter(
            (id, index) =>
              !(body.tracks_positions || []).includes(index) &&
              !(body.track_ids || []).includes(id),
          );
        else p.name = body.name || p.name;
      }
      return json(res, p);
    }
    if (path === "/v1/content/featured/albums")
      return json(res, { albums: list(albums.slice(0, 6)), hero_index: 0 });
    if (path === "/v1/content/popular")
      return json(res, { albums: list(albums), artists: list(artists) });
    if (path === "/v1/user/listening/history")
      return json(res, {
        entries: list(
          albums.slice(0, 6).map((a) => ({
            album_id: a.id,
            album_name: a.name,
            artist_name: a.artist_names[0],
            track_id: tracks.find((t) => t.album_id === a.id).id,
            started_at: 1780000000,
          })),
        ),
      });
    if (path === "/v1/content/genres") return json(res, list(genres));
    if (path.startsWith("/v1/content/genre/"))
      return json(res, {
        artwork_url:
          genres.find(
            (g) =>
              g.name ===
              decodeURIComponent(
                path.slice("/v1/content/genre/".length).split("/")[0],
              ),
          )?.artwork_url || null,
        track_ids: list(tracks.slice(0, 24).map((t) => t.id)),
        total: 24,
        has_more: false,
      });
    if (path === "/v1/content/works") return json(res, list([work]));
    if (path === "/v1/content/work/work-1")
      return json(res, {
        work,
        relations: [],
        tracks: list(
          tracks.slice(0, 6).map((track) => ({
            track,
            album: albums[0],
            artists: [artists[0]],
            relationship_scope: "direct",
            recording_work: work,
          })),
        ),
        has_more: false,
        next_offset: 6,
      });
    if (path === "/v1/content/concepts")
      return json(
        res,
        list(
          genres.map((g) => ({
            id: `genre:${g.name}`,
            entity_id: `genre:${g.name}`,
            label: g.name,
            family: "genre_tag",
            name: g.name,
            track_count: 24,
          })),
        ),
      );
    if (path === "/v1/content/search")
      return json(res, search(body.query || ""));
    if (path === "/v1/content/search/stream") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-store",
      });
      return res.end(
        `data: ${JSON.stringify({ section: "results", items: search(url.searchParams.get("q") || "") })}\n\ndata: {"section":"done"}\n\n`,
      );
    }
    const item = path.match(
      /^\/v1\/content\/(album|artist|track)\/([^/]+)(?:\/(.+))?$/,
    );
    if (item) {
      const [, , id, suffix] = item,
        kind = item[1];
      const value = { album: albums, artist: artists, track: tracks }[
        kind
      ].find((x) => x.id === id);
      if (!value) return json(res, { error: "Unknown fixture ID" }, 404);
      if (suffix === "lyrics")
        return json(
          res,
          state.scenario === "empty"
            ? null
            : {
                track_id: id,
                status: "found",
                provider: "mock",
                plain_lyrics:
                  "The city wakes in amber light\nWe leave the quiet streets behind\nAn open window, morning air\nA little time for us to share",
                synced_lyrics: Array.from(
                  { length: 18 },
                  (_, i) =>
                    `[${String(Math.floor((i * 5) / 60)).padStart(2, "0")}:${String((i * 5) % 60).padStart(2, "0")}.00]${["The city wakes in amber light", "We leave the quiet streets behind", "An open window, morning air", "A little time for us to share"][i % 4]}`,
                ).join("\n"),
                fetched_at: 1780000000,
              },
        );
      if (suffix === "discography")
        return json(
          res,
          paged(
            albums.filter((a) => a.artists_ids.includes(id)),
            "albums",
          ),
        );
      if (suffix === "greatest-hits")
        return json(res, {
          track_ids: tracks
            .filter((t) => t.artists_ids.includes(id))
            .map((t) => t.id),
        });
      return json(
        res,
        kind === "album"
          ? suffix === "resolved"
            ? resolvedAlbum(value)
            : value
          : kind === "track"
            ? suffix === "resolved"
              ? resolvedTrack(value)
              : value
            : {
                artist: value,
                related_artists: artists.filter((a) => a.id !== id),
                display_image: { id },
                enrichment: { biography: value.biography },
              },
      );
    }
    if (path.includes("/lyrics/") && path.endsWith("/download"))
      return json(res, { tracks: 1 });
    if (path === "/v1/content/radio/options")
      return json(res, {
        default_recipe_id: "balanced",
        recipes: [
          {
            id: "classic",
            name: "Classic",
            mode: "similar",
            diversity: 0.2,
            randomness: 0.3,
            criteria: [{ namespace: "musicfm.mean.v1", weight: 1 }],
          },
          {
            id: "balanced",
            name: "Balanced",
            mode: "similar",
            diversity: 0.3,
            randomness: 0.3,
            criteria: [
              { namespace: "musicfm.mean.v1", weight: 0.55 },
              { namespace: "ast.audioset.v2", weight: 0.3 },
              { namespace: "ast.instruments.v2", weight: 0.15 },
            ],
          },
        ],
        criteria: [
          { namespace: "musicfm.mean.v1", label: "Sound profile" },
          { namespace: "ast.audioset.v2", label: "Audio scene" },
          { namespace: "ast.instruments.v2", label: "Instrumentation" },
        ],
        modes: ["similar", "explore"],
        explicit_filters: ["include", "exclude", "only"],
        count: { min: 1, max: 200, default: 50 },
        diversity: { min: 0, max: 1, default: 0.3 },
        randomness: { min: 0, max: 1, default: 0.3 },
      });
    if (
      path.includes("/radio/") ||
      path === "/v1/content/recommendations/continuation"
    )
      return json(res, { track_ids: tracks.slice(6, 24).map((t) => t.id) });
    if (path === "/v1/user/devices/1/share_policy" && method === "PUT") {
      deviceSharePolicy = body;
      return json(res, deviceSharePolicy);
    }
    if (path === "/v1/user/devices")
      return json(res, {
        devices: [
          {
            id: 1,
            device_id: 1,
            name: "Design browser",
            device_name: "Design browser",
            device_type: "web",
            share_policy: deviceSharePolicy,
            is_online: true,
            last_seen: 1780000000,
          },
          {
            id: 2,
            device_id: 2,
            name: "Living room speaker",
            device_name: "Living room speaker",
            device_type: "android",
            share_policy: {
              mode: "deny_everyone",
              allow_users: [],
              deny_users: [],
              allow_roles: [],
            },
            is_online: true,
            last_seen: 1780000000,
          },
        ],
      });
    if (
      path === "/v1/user/impression" ||
      (path.includes("/listening") && method === "POST")
    )
      return json(res, { ok: true });
    if (path === "/v1/download/limits")
      return json(res, {
        max_pending_requests: 20,
        pending_requests: 0,
        requests_today: 2,
        max_per_day: 20,
        can_request: true,
      });
    if (path === "/v1/download/status")
      return json(res, { enabled: true, available: true });
    if (path === "/v1/download/my-requests")
      return json(res, {
        requests: list(requests),
        total: list(requests).length,
      });
    if (path.startsWith("/v1/download/request/")) {
      const id = body.album_id || body.track_id;
      const old = requests.find((r) => r.content_id === id);
      const request = {
        id: old?.id || `request-${id}`,
        content_id: id,
        content_type: body.album_id ? "ALBUM" : "TRACK",
        content_name: body.album_name || "Track",
        status: "PENDING",
        priority: "USER",
        created_at: 1780000000,
        queue_position: 2,
      };
      if (old) Object.assign(old, request);
      else requests.push(request);
      return json(res, {
        success: true,
        request_id: request.id,
        status: "pending",
        queue_position: 2,
      });
    }
    if (path === "/v1/admin/users")
      return json(
        res,
        list([
          {
            handle: "design-demo",
            user_handle: "design-demo",
            roles: ["admin"],
            permissions,
            created_at: 1780000000,
          },
        ]),
      );
    if (path.endsWith("/roles")) return json(res, ["admin"]);
    if (path.endsWith("/permissions")) return json(res, permissions);
    if (path.endsWith("/credentials"))
      return json(res, { has_password: true, oidc_subject: null });
    if (path === "/v1/admin/listening/daily")
      return json(
        res,
        list(
          Array.from({ length: 14 }, (_, i) => ({
            date: 20261001 + i,
            total_duration_seconds: 3600 + i * 120,
            total_plays: 30 + i,
            unique_users: 3,
            unique_tracks: 12,
            completed_plays: 25 + i,
          })),
        ),
      );
    if (path === "/v1/admin/listening/top-tracks")
      return json(
        res,
        list(
          tracks.slice(0, 8).map((t, i) => ({
            track_id: t.id,
            play_count: 30 - i,
            total_duration_seconds: 3600 - i * 120,
          })),
        ),
      );
    if (path === "/v1/admin/online-users")
      return json(res, { count: 1, users: ["design-demo"] });
    if (path === "/v1/admin/storage")
      return json(res, {
        total_bytes: 1234567890,
        database_total_bytes: 12345678,
        filesystem_total_bytes: 1222222212,
        databases: [],
        components: [],
      });
    if (path === "/v1/admin/embeddings/coverage")
      return json(res, {
        coverage: {
          available_tracks: 66,
          fully_embedded_tracks: 60,
          tracks_missing_any_embedding: 6,
          namespaces: [],
        },
      });
    if (path === "/v1/admin/jobs")
      return json(res, {
        jobs: list([
          {
            id: "metadata_enrichment_v1",
            name: "Metadata enrichment",
            description: "Local fixture job",
            enabled: true,
            is_running: false,
            last_run: null,
            schedule: "Every hour",
          },
        ]),
      });
    if (path === "/v1/admin/search/relevance-filter")
      return json(res, { config: { method: "none", threshold: 0.5 } });
    if (path.includes("/audit")) return json(res, { entries: [], total: 0 });
    if (path === "/v1/admin/push/registrations")
      return json(res, {
        enabled: true,
        registrations: list([
          {
            id: "push-1",
            device_name: "Living room speaker",
            device_type: "android",
            user_handle: "design-demo",
            connected: true,
            endpoint_host: "mock.local",
            created_at: 1780000000,
          },
        ]),
      });
    if (path === "/v1/admin/changelog/batches")
      return json(
        res,
        list([
          {
            id: "batch-1",
            name: "Autumn arrivals",
            description: "Six new releases for the local catalog.",
            is_open: true,
            created_at: 1780000000,
            last_activity_at: 1780000000,
          },
        ]),
      );
    if (path === "/v1/admin/bug-reports")
      return json(
        res,
        list([
          {
            id: "report-1",
            title: "Lyrics follow-up example",
            user_handle: "design-demo",
            client_type: "web",
            created_at: 1780000000000,
            size_bytes: 2048,
          },
        ]),
      );
    if (path === "/v1/download/admin/proxy")
      return json(res, {
        active: [],
        recent: [],
        foreground_active: 0,
        foreground_limit: 4,
        prefetch_active: 0,
        prefetch_limit: 2,
        memory_used_bytes: 0,
        memory_limit_bytes: 536870912,
      });
    if (path === "/v1/download/admin/stats")
      return json(res, {
        pending: 0,
        in_progress: 0,
        completed: 72,
        failed: 2,
        queue_size: 0,
      });
    if (path === "/v1/download/admin/stats/history")
      return json(res, { points: [] });
    if (path === "/v1/download/admin/requests")
      return json(
        res,
        list(
          requests.filter((r) =>
            (url.searchParams.get("status") || "PENDING,IN_PROGRESS").includes(
              r.status,
            ),
          ),
        ),
      );
    if (path === "/v1/download/admin/activity") return json(res, []);
    if (path === "/v1/ingestion/my-jobs" || path === "/v1/ingestion/admin/jobs")
      return json(res, list(ingestionJobs));
    if (path === "/v1/ingestion/reviews") return json(res, []);
    misses.add(`${method} ${path}`);
    console.warn(`[mock] Unhandled: ${method} ${path}`);
    return json(res, { error: `Not simulated: ${method} ${path}` }, 501);
  }
  return {
    name: "local-design-mock",
    apply: "serve",
    enforce: "pre",
    resolveId(id) {
      if (/(?:^|\/)oidc(?:\.js)?$/.test(id))
        return fileURLToPath(new URL("./oidc.js", import.meta.url));
    },
    transformIndexHtml() {
      return [
        {
          tag: "script",
          injectTo: "head-prepend",
          children: `if(!localStorage.getItem('playlistsHistory')&&${state.scenario !== "empty" && state.scenario !== "signed-out"}){localStorage.setItem('playlistsHistory',JSON.stringify([{type:'ALBUM',context:{id:'album-1',name:'Golden Hour',edited:false},tracksIds:['track-1','track-2','track-3','track-4','track-5','track-6']}]));localStorage.setItem('currentPlaylistIndex','0');localStorage.setItem('currentTrackIndex','0');localStorage.setItem('progressSec','12');localStorage.setItem('progressPercent','0.133');}`,
        },
        {
          tag: "script",
          attrs: { type: "module" },
          children: `const a=document.createElement('a');a.href='/__mock';a.textContent='MOCK · Design lab';a.style='position:fixed;bottom:4px;left:4px;z-index:9999;background:#1ed760;color:#071108;padding:4px 8px;border-radius:4px;font:11px system-ui;text-decoration:none';document.body.append(a);`,
          injectTo: "body",
        },
      ];
    },
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        handle(req, res, next).catch((e) => {
          console.error("[mock]", e);
          if (!res.headersSent) json(res, { error: e.message }, 500);
          else res.end();
        });
      });
      const wss = new WebSocketServer({ noServer: true });
      const upgrade = (req, socket, head) => {
        if (req.url?.split("?")[0] !== "/v1/ws") return;
        wss.handleUpgrade(req, socket, head, (ws) =>
          wss.emit("connection", ws),
        );
      };
      server.httpServer.on("upgrade", upgrade);
      wss.on("connection", (ws) => {
        const send = (type, payload) =>
          ws.send(JSON.stringify({ type, payload }));
        send("connected", { device_id: 1, server_version: "mock" });
        ws.on("message", (raw) => {
          try {
            const m = JSON.parse(raw);
            if (m.type === "ping") send("pong", null);
            if (m.type === "playback.hello")
              send("playback.welcome", {
                device_id: 1,
                devices: [
                  {
                    id: 3,
                    name: "Kitchen tablet",
                    device_type: "android",
                    is_shared: true,
                    owner_handle: "alex",
                  },
                  {
                    id: 1,
                    device_id: 1,
                    name: "Design browser",
                    device_name: "Design browser",
                    device_type: "web",
                  },
                  {
                    id: 2,
                    device_id: 2,
                    name: "Living room speaker",
                    device_name: "Living room speaker",
                    device_type: "android",
                  },
                ],
                session: {
                  active_devices: [
                    {
                      device_id: 2,
                      device_name: "Living room speaker",
                      state: {
                        is_playing: true,
                        position: 32,
                        timestamp: Date.now(),
                        current_track: {
                          id: "track-7",
                          title: "Sunday in Rome",
                          artist_name: "Luca Moretti",
                          image_id: "album-2",
                          duration: 210000,
                        },
                      },
                    },
                  ],
                },
              });
          } catch {
            ws.close(1003);
          }
        });
      });
      server.httpServer.once("close", () => {
        for (const ws of wss.clients) ws.terminate();
        wss.close();
      });
    },
  };
}
