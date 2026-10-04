# UnifiedPush

Pezzottify supports [UnifiedPush](https://unifiedpush.org) so the server can wake the
Android app when something notification-worthy happens, instead of waiting for the
next periodic background sync. Any UnifiedPush distributor works (ntfy, NextPush,
LelloStore once it implements the distributor protocol). Pezzottify has no code
specific to a distributor.

## Design: a push is a wake-up, not the notification

The sync event log stays the single source of truth. A push carries no user
content: it tells the app "there are new events for you". The app then runs its
normal authenticated sync catch-up (the same path as `BackgroundSyncWorker`), and
notifications are shown from the synced events exactly as today. Consequences:

- Lost, duplicated or delayed pushes are harmless; the next sync catches up.
- Distributors never see notification content.
- Without a distributor the app keeps polling as before.

## Server

### VAPID

The server signs pushes with a VAPID key (RFC 8292). The P-256 private key is read
from `push.vapid_private_key_file` (PEM) or, if unset, generated once and stored as
`vapid_private_key.pem` in the database directory. `push.vapid_subject` defaults to
`mailto:pezzottify@localhost`.

### API (authenticated, any logged-in user)

| Method/path | Body | Purpose |
| --- | --- | --- |
| `GET /v1/push/vapid` | - | `{"public_key": "<base64url uncompressed P-256 point, 87 chars>"}` |
| `PUT /v1/push/registrations` | `{"endpoint", "p256dh", "auth", "device_id"?}` | Upsert this device's registration (keyed by endpoint) |
| `DELETE /v1/push/registrations` | `{"endpoint"}` | Remove a registration owned by the caller |

`endpoint` must be an `https://` URL (or `http://` only when
`push.allow_insecure_endpoints = true`, for local testing), at most 2048 bytes.
`p256dh` and `auth` are the base64url Web Push keys from the distributor's
`PublicKeySet` (`pubKey`, `auth`). An endpoint registered by another user is moved
to the caller (the device changed account). At most 20 registrations per user; the
oldest is evicted. `device_id` is optional, at most 128 characters.

`PUT` and `DELETE` answer `204 No Content`. `DELETE` is idempotent (an unknown
endpoint, or one owned by another user, is a no-op). Invalid input answers `400`
with code `invalid_push_registration` (also for endpoints carrying credentials, and
keys that are not a 65-byte uncompressed P-256 point / 16-byte auth secret). The
routes share the per-user write rate limit.

### Sending

Every user event can wake the user's other devices, at two speeds:

| Events | Window | Urgency | TTL | Topic |
| --- | --- | --- | --- | --- |
| `notification_created`, `whatsnew_batch_closed` | 2 s | normal | 86400 | - |
| everything else (likes, playlists, settings, permissions, download requests and status, notification read) | 60 s | low | 3600 | `sync` |
| `download_progress_updated` | never | | | |

A window opens with the first event and every event during it folds in, so
continuous editing on one device wakes the others at most once a minute rather
than once per change (a pure debounce would starve during long edits). A
notification arriving during an open sync window shortens it to 2 s and is sent
with notification settings. Download progress is excluded: it repeats for the
whole download and completion has its own notification.

Devices with a live WebSocket are skipped: they already received the events.
This uses the registration's `device_id` (the device UUID) and the WebSocket
connection list.

- `POST <endpoint>`, `Content-Encoding: aes128gcm` (RFC 8291), VAPID
  `Authorization: vapid t=..., k=...`, `TTL` and `Urgency` as above, and
  `Topic: sync` for sync wake-ups so a push service can replace an undelivered
  one with the newer one (RFC 8030 section 5.4).
- Payload (encrypted): `{"type":"sync","seq":<latest user event sequence>}`.
- `404`/`410`: the registration is deleted. Other failures are logged and counted;
  a registration failing for 7 consecutive days is deleted.

Push is enabled when the `[push]` section exists (`enabled = true` by default
there); otherwise the endpoints return `503` and nothing is sent.

## Android

- Library: `org.unifiedpush.android:connector`.
- `PushServiceImpl` (extends `PushService`, declared non-exported with the
  `org.unifiedpush.android.connector.PUSH_EVENT` action):
  - `onNewEndpoint`: send `PUT /v1/push/registrations` (retried with WorkManager).
  - `onMessage`: enqueue an expedited one-time sync (unique work, keep existing).
  - `onUnregistered`: `DELETE /v1/push/registrations`; fall back to polling.
- Registration happens after login and at startup when a distributor is
  available, with the VAPID key from `GET /v1/push/vapid`. Logout unregisters.
- Settings → Notifications shows the push status (distributor in use, none
  installed, or off) and lets the user pick a distributor when several exist.
- Adding the service changes the manifest, so it ships in a new Paravoid shell APK.
