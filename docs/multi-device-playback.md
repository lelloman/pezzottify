# Multi-Device Playback

**Status**: Implemented (per-device state relay and remote control). Playback transfer is not planned; see "Known gaps".

This document is the wire contract between the server's playback session manager
(`pezzottify-server/src/server/websocket/playback_session.rs`) and the web and
Android clients. Clients and server must agree on everything in here. When the
behaviour changes, change this file in the same commit.

## Concepts

- **Device**: one installation of a client, identified by a client-generated
  UUID that is persisted on the device and sent at login. The server maps the
  UUID to a numeric `device_id` and stores it on the session. A WebSocket
  connection without a device on the session is rejected with HTTP 400.
- **Session**: the server keeps, per user, the latest reported playback state
  and queue of each connected device. The store is in memory only; a server
  restart forgets everything and clients re-announce on reconnect.
- **No active device**: the server has no notion of which device is "the"
  player. Every device plays its own audio and reports its own state. Any
  device may send commands to any other device it can see.
- **Controller (remote mode)**: a client state in which the local player is
  idle and the UI drives another device through commands. It is purely a client
  concept. A controller must not execute commands it receives, because it is
  not reporting any state that the sender could observe.
- **Shared device**: a device whose owner set a share policy that lets other
  users see it and control it. Visibility and control use the same policy.

## Device identity

| Platform | UUID | Name | Type |
|---|---|---|---|
| Web | `localStorage["pezzottify_device_id"]`, created once | "Browser on OS" detected from the user agent | `web` |
| Android phone | `SharedPreferences pezzottify_device/device_uuid`, `android-<uuid>` | `MANUFACTURER MODEL` | `android` |
| Android TV | same as phone | same as phone | `android_tv` |

Identity reaches the server in one of three ways:

1. `POST /v1/auth/login` body: `device_uuid`, `device_type`, `device_name`, `os_info`.
2. OIDC authorize request extra parameters `device_id`, `device_type`,
   `device_name`. The identity provider must copy them into the ID token as
   custom claims with the same names. The server reads them from the token on
   every request.
3. The TV device-code flow sends the same three parameters.

Both web login paths must use the same persisted UUID and the same name
detector. A fresh UUID per login creates a new device row every time and
pollutes the device list.

Related HTTP endpoints (own devices only):

- `GET /v1/user/devices`: every registered device of the user, online or not,
  with its share policy.
- `PUT /v1/user/devices/{device_id}/share_policy`: body
  `{ "mode": "allow_everyone" | "deny_everyone" | "custom", "allow_users": [], "deny_users": [], "allow_roles": [] }`.
  The server then pushes a `playback.device_list_changed` with change type
  `policy_changed` to the owner and to every connected user, so users who lost
  access drop the device too.

The admin endpoint `GET /v1/admin/playback/sessions` returns a snapshot of all
active device states.

## Transport

WebSocket at `GET /v1/ws`. Envelope in both directions:

```json
{ "type": "playback.state", "payload": { } }
```

Messages whose type starts with `playback.` are routed to the playback session
manager. Any failure is answered to the sender only with `playback.error`.

The server keeps one connection per `(user_id, device_id)`. A new connection for
the same device replaces the old one. The old socket's cleanup must not remove
the new registration or its playback state (the server tags each registration
with a generation and only cleans up when it still matches).

## Units and types

- `position` and `duration` are **seconds** as JSON numbers (`f64`), on every
  platform. Never milliseconds.
- `timestamp` is the sender's Unix time in milliseconds. Receivers use it only
  for ordering and diagnostics. Receivers interpolate progress from their own
  local arrival time, never from the sender's clock, because device clocks
  are not synchronized.
- `volume` is `0.0..=1.0`.
- `repeat` is `"off" | "all" | "one"`.
- `device_type` is `"web" | "android" | "android_tv" | "ios"`.
- Device ids are the server's numeric ids.

### PlaybackTrack

```json
{
  "id": "track-id",
  "title": "Title",
  "artist_id": "primary-artist-id",
  "artist_name": "Primary Artist",
  "artists_ids": ["primary-artist-id", "featured-artist-id"],
  "album_id": "album-id",
  "album_title": "Album",
  "duration": 183.5,
  "track_number": 3,
  "image_id": "image-id"
}
```

`artists_ids` carries every artist of the track, primary first. `track_number`
and `image_id` are optional.

### PlaybackState

```json
{
  "current_track": { PlaybackTrack } | null,
  "queue_position": 0,
  "queue_version": 12,
  "position": 42.0,
  "is_playing": true,
  "volume": 0.8,
  "muted": false,
  "shuffle": false,
  "repeat": "off",
  "timestamp": 1730000000000
}
```

A state with `current_track: null` and `is_playing: false` is the **stop
marker**. The server removes the device's session entry and tells peers the
device stopped. Clients must only send it when they really stopped. A client
whose track metadata has not resolved yet must skip that broadcast instead of
sending a null track.

### QueueItem and context

```json
{ "id": "track-id", "added_at": 1730000000000 }
```

`context` is an opaque JSON object describing where the queue came from
(album, playlist, user mix, radio) plus smart-continuation data such as
`continuation` and `gravity`. The server stores and relays it unchanged. Its
shape is owned by the clients and documented in `docs/radio-playback.md` and
`docs/steering-concepts.md`.

## Client to server messages

| Type | Payload | When |
|---|---|---|
| `playback.hello` | `{ device_name, device_type }` | Once per connection, right after `connected`. Retried every 2 s, up to 10 attempts, until a welcome arrives. An error before the welcome stops the retries. |
| `playback.state` | `PlaybackState` | On every local change (play, pause, seek, skip, volume, mute, track change) and every 5 s while the device has a loaded track. Not sent while in controller mode. |
| `playback.queue_update` | `{ queue: [QueueItem], queue_version, context? }` | Whenever the local queue or its context changes, and once when the device starts reporting (first hello after load or reconnect), so peers never have a state without a queue. Max 500 items. |
| `playback.command` | `{ command, payload, target_device_id }` | To control another device. `target_device_id` is required. |
| `playback.request_queue` | `{ target_device_id? }` | Ask for a device's current queue. Reply is `playback.queue_sync`. |

## Server to client messages

| Type | Payload | When |
|---|---|---|
| `playback.welcome` | `{ device_id, devices: [ConnectedDevice], session: { active_devices: [DevicePlaybackInfo] } }` | Reply to hello. `active_devices` holds state, queue, `queue_version` and `context` for every device the recipient may see that currently has a session entry. |
| `playback.device_list_changed` | `{ devices: [ConnectedDevice], change: { type, device_id } }` | A device connected, disconnected, started or stopped reporting, flipped between playing and paused, went stale, or its share policy changed. `type` is one of `connected`, `disconnected`, `started_playing`, `stopped_playing`, `playback_changed`, `stale`, `policy_changed`. Each recipient gets the list filtered for what they may see. |
| `playback.device_state` | `{ device_id, device_name, state }` | Relay of another device's `playback.state`. |
| `playback.device_queue` | `{ device_id, queue, queue_version, context? }` | Relay of another device's `playback.queue_update`. |
| `playback.device_stopped` | `{ device_id, reason }` | Another device sent the stop marker (`stopped`), disconnected (`disconnected`) or stopped reporting for 120 s (`stale`). |
| `playback.queue_sync` | `{ device_id, queue, queue_version, context? }` | Reply to `playback.request_queue`. |
| `playback.command` | `{ command, payload }` | A command forwarded to this device. `target_device_id` is stripped. |
| `playback.error` | `{ code, message, context? }` | Reply to the sender of a bad message. Codes: `invalid_message`, `queue_limit_exceeded`, `command_failed`, `forbidden`, `device_not_found`, `unknown_command`. Clients must surface forbidden, device_not_found and unknown_command to the user. |

### ConnectedDevice

```json
{
  "id": 42,
  "name": "Pixel 8",
  "device_type": "android",
  "is_playing": true,
  "has_session": true,
  "connected_at": 1730000000000,
  "owner_user_id": 1,
  "owner_handle": "alice",
  "is_shared": false
}
```

`has_session` is true when the device has reported a state and has not stopped.
`is_playing` is the device's last reported `is_playing`. A paused device has
`has_session: true, is_playing: false`.

## Commands

The server accepts only the command names below and answers anything else with
`playback.error` code `unknown_command`. Payload fields are listed in
parentheses. Positions and indexes are zero based.

| Command | Payload | Web target | Android target |
|---|---|---|---|
| `play` | | yes | yes |
| `pause` | | yes | yes |
| `next` | | yes | yes |
| `prev` | | yes | yes |
| `seek` | `position` (seconds) | yes | yes |
| `setVolume` | `volume` | yes | yes |
| `setMuted` | `muted` | yes | yes |
| `loadAlbum` | `albumId`, `startTrackId?` | yes | yes |
| `loadPlaylist` | `playlistId`, `startTrackId?` | yes | yes |
| `loadSingleTrack` | `trackId` | yes | yes |
| `loadTrackIds` | `trackIds`, `context?` | yes | yes |
| `addAlbumToQueue` | `albumId` | yes | yes |
| `addPlaylistToQueue` | `playlistId` | yes | yes |
| `addTracksToQueue` | `trackIds` | yes | yes |
| `skipToTrack` | `index` | yes | yes |
| `removeTrack` | `index` or `trackId` | yes | yes |
| `moveTrack` | `fromIndex`, `toIndex` | yes | yes |
| `setGravity` | `gravity` | yes | yes |
| `setShuffle` | `enabled` | no (web player has no shuffle) | yes |
| `setRepeat` | `mode` | no (web player has no repeat) | yes |

A target that receives a command it cannot honour ignores it and logs it. It
still broadcasts its state afterwards, so the sender sees that nothing changed.
A device in controller mode ignores every incoming command.

After executing a command the target broadcasts `playback.state` immediately,
and `playback.queue_update` if the queue changed.

## Lifecycle on each platform

### Server

- `hello` stores device metadata (name, type, owner handle, connected_at) and
  broadcasts `connected`.
- `state` upserts the session entry. The first state from a device broadcasts
  `started_playing`; a flip of `is_playing` broadcasts `playback_changed`; the
  stop marker removes the entry and broadcasts `device_stopped` plus
  `stopped_playing`.
- Disconnect removes metadata and session entry and broadcasts
  `device_stopped` (if there was an entry) and `disconnected`.
- A maintenance task runs every 5 s and removes entries not updated for 120 s,
  broadcasting `device_stopped` with reason `stale` and a
  `device_list_changed` with type `stale`.
- Share policies are cached in memory per device and refreshed when the owner
  changes them, so relaying a state does not hit the database. The default
  policy is deny everyone, which short-circuits without any lookup.

### Web

- Each tab is its own device and owns its own audio player.
- The session store (`web/src/store/playbackSession.js`) sends hello on every
  connect, reports state and queue, and keeps `devices` and
  `otherDeviceStates` (state, queue, context, local `receivedAt`).
- The Devices page shows every visible device with play, pause, skip and seek
  controls that send commands. The bottom-bar device selector lists devices.
- The WebSocket keeps pinging and reconnecting while the tab is hidden if the
  tab is playing or reporting, so a background tab stays reachable.
- The remote-mode branch of the playback store is not wired to the UI.

### Android

- The phone can enter controller mode from the Devices screen. The router
  switches every player flow to `RemotePlaybackController`, the local player
  session is cleared, and content actions become commands. Leaving controller
  mode pauses the remote device.
- Controller mode survives WebSocket reconnects. It exits only when a fresh
  device list from the server no longer contains the target.
- One foreground service, `PlaybackService`, backs every media control. It runs
  while any media session has content, whether local or remote, playing or
  paused. In controller mode it hosts a second media session whose player
  mirrors the target, so the notification, lock screen and headset controls
  drive the target exactly as they drive local playback.
- The WebSocket stays connected while any media session is active, in the
  foreground or not. A paused device therefore stays reachable, and it drops
  off other devices' lists only once its session ends and the app is in the
  background.
- Swiping the app away pauses and ends everything: local playback, or the target
  in controller mode, following the rule that leaving controller mode pauses it.
- The TV build is a target only. It has no Devices screen and shows a
  "waiting for playback" card with its device name and connection status.

## Known gaps

- **No playback transfer.** Entering controller mode on Android wipes the local
  queue instead of handing it to the target, and leaving it pauses the target
  instead of pulling playback back. The feature was considered and shelved.
- **No capability negotiation.** The web player has no shuffle or repeat, so
  those commands do nothing there. A future `hello` could advertise supported
  commands so controllers can hide unsupported controls.
- **No command acknowledgement.** The sender learns the outcome only from the
  target's next state broadcast.
- **No persistence.** Device sessions live in server memory only.
