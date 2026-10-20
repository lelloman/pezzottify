# Maintenance mode

The server owns the maintenance flag. It is stored as JSON under
`server.maintenance.v1` in `server.db`, so it survives restarts and can be
changed without redeploying.

## API

- `GET /v1/status` is public and always available. It returns:

  ```json
  {
    "enabled": true,
    "message": "Back after the upgrade",
    "estimated_end": "2026-10-21T08:00:00Z"
  }
  ```

  `message` and `estimated_end` are optional. The server normalizes
  `estimated_end` to RFC 3339 UTC.

- `GET /v1/admin/maintenance` reads the same state.
- `PUT /v1/admin/maintenance` updates it (requires `ServerAdmin`):

  ```json
  {
    "enabled": true,
    "message": "Back after the upgrade",
    "estimated_end": "2026-10-21T10:00:00+02:00"
  }
  ```

  The admin endpoint returns the stored state. Invalid timestamps are rejected
  with `400`; persistence failure leaves the previous in-memory state intact.

## Request gating

While enabled, `/v1/*` player requests return `503` with the maintenance state
as the body and an `X-Maintenance-Mode: 1` header. `/v1/status`, `/v1/auth/*`
and `/v1/admin/*` stay reachable so clients can show the banner and a server
admin can turn the flag off from `/admin/maintenance`.

## Clients

- Web replaces the normal shell with a full-screen maintenance view (except for
  `/login`, `/auth/callback` and `/admin/*`). It polls `/v1/status` and also
  reacts to the `503` marker from any failed request.
- Android checks `/v1/status` at startup and switches to a full-screen
  maintenance screen that only offers logout. A `503` marker from any API call
  likewise replaces the normal shell mid-session. TV uses the same screen.

## Server swaps

The flag belongs to the server the client is configured to use. Swapping
servers therefore hands maintenance control to the new server. A message that
must survive a server being completely unreachable needs an external layer
(reverse proxy, DNS or an app-store notice) and is out of scope here.
