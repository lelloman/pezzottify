# Local UI design lab

Run the **real Vue application**, with Vite hot reload and a local in-memory mock API:

```sh
cd web
npm ci
npm run dev:mock
```

- App: http://127.0.0.1:5174/
- Screen index and scenarios: http://127.0.0.1:5174/__mock
- Request coverage: http://127.0.0.1:5174/__mock/status

No Rust server, database, homelab, credentials, or identity provider is needed.
The mock runs inside Vite's development server. `/v1` is handled locally, with
no fallback proxy. Normal `npm run dev` still proxies to localhost:3001, and
`npm run build` does not load the mock plugin or its identity adapter.
The dedicated port keeps demo browser storage separate from normal development.
The service worker is disabled in mock mode so cached data cannot mask changes.

## Iteration loop

1. Open a screen from the index, or navigate normally in the app.
2. Edit the actual components/styles under `src/`; Vite updates them immediately.
3. Edit `mock/fixtures.js` for catalog/artwork; `mock/plugin.js` for API responses.
   Vite restarts when these config dependencies change.
4. Use the index to reset data or select `populated`, `empty`, `slow` (1.8-second
   API delay), `error` (503), or `signed-out`. Applying a scenario clears browser
   storage on this mock origin and resets server state. Scenarios are shared
   between tabs on the same mock server.

The seed has 6 fictional artists, 12 albums, 72 tracks, playlists, a composition,
genres, sample administrative records, generated SVG artwork, and original
sample lyrics. A paused album queue is preloaded for Now playing and Steering.
Playback uses a quiet, generated 90-second test tone with byte-range seeking.
It is not the artist's music. The login form accepts any non-empty values;
the SSO button uses a local adapter and exercises the callback route.

## Coverage and limits

The screen index covers every user and admin page: home, search, album, artist,
track, composition, playlist, genres, genre detail, player, steering, devices,
settings, download requests, users, analytics, server, downloads, batches,
bug reports, ingestion, push, and login. The actual components also expose their
normal dialogs and menus.

Likes, playlist creation/renaming/deletion/track edits, and settings mutate local
memory. Search, lyrics, playback, and the playback WebSocket handshake work.
Admin records are UI fixtures: this is **not a full backend emulator**. Upload
processing, real downloads, AI generation/MCP, actual push delivery, remote-device
control, and most administrative mutations are not simulated. Unsupported HTTP
requests return 501 and appear in `/__mock/status`, rather than pretending to
succeed or contacting production. Add an explicit handler when an iteration
needs another interaction. State resets when the server restarts.

## Validation

```sh
npx playwright install chromium  # once, if needed
npm run test:mock
npm run build
```

The smoke test launches its own separate server, visits the real screen routes,
checks for browser exceptions and external requests, exercises playback, local
login/callback, playlist edits and likes, and checks scenario behavior. It does
not reset the server you are using for design work.
