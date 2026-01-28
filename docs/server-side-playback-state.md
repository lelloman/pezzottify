# Server-Side Playback State

## Overview

Enable synchronized playback control across all of a user's connected devices. When a user plays music on one device, all other connected clients see the same player state and can control playback remotely.

## Architecture: Relay Model

The audio device owns the playback state. The server acts as a relay, broadcasting state to other clients and forwarding commands back to the audio device.

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│  Audio Device   │ ──────▶ │     Server      │ ──────▶ │ Remote Clients  │
│  (owns state)   │         │    (relay)      │         │ (view/control)  │
│                 │ ◀────── │                 │ ◀────── │                 │
│ plays audio     │ commands│ broadcasts to   │ commands│ see player UI   │
│ broadcasts state│         │ user's clients  │         │ send commands   │
└─────────────────┘         └─────────────────┘         └─────────────────┘
```

## Key Concepts

### Audio Device
- The client that is currently playing audio
- Source of truth for playback state
- Periodically broadcasts state updates to server
- Receives and executes commands from remote clients
- Any client can become the audio device via device selector in player UI
- Identified by auto-generated name: device type + browser/OS info (e.g., "Chrome on Windows", "Android Phone")
- **Multiple instances**: Each browser tab or app instance is a separate client with its own device ID. If a user has two Chrome tabs open, they appear as two separate devices (e.g., "Chrome on Windows", "Chrome on Windows (2)"). The server appends a number suffix when device names collide for the same user.

### Remote Client
- A connected client that is NOT the audio device
- Displays the player UI mirroring the audio device's state
- Can send control commands (play, pause, seek, skip, etc.)
- Shows device selector to switch audio output to any connected device
- Has full access to queue (synced on queue changes)

### Playback Session
- Exists as long as an audio device is connected
- Contains: current track, queue, position, play/pause state, volume, shuffle, repeat
- Cleared when audio device disconnects (no persistence)
- One session per user (single user = single session)
- **Queue limit**: 500 tracks maximum (hard limit, ~30KB payload)

## Behaviors

### Client Connection

When a client connects via WebSocket:

1. Client sends `hello` message with device info
2. Server responds with `welcome` containing:
   - Assigned device ID
   - Current session state (if exists)
   - List of connected devices

```typescript
// Client → Server
{
  type: "hello",
  deviceName: string,  // auto-generated: "Chrome on Windows"
  deviceType: "web" | "android" | "ios"
}

// Server → Client
{
  type: "welcome",
  deviceId: string,
  session: {
    exists: boolean,
    state?: PlaybackState,
    queue?: QueueItem[],
    audioDeviceId?: string,
    reclaimable?: boolean  // true if this device was recently audio device
  },
  devices: ConnectedDevice[]
}
```

### Session Lifecycle

1. **Session Start**: User initiates playback on a client with no active session → that client becomes audio device → session created → other connected clients see remote player UI
2. **Session Active**: Audio device broadcasts state → server relays to remote clients → remote clients display player with device selector
3. **Session End**: Audio device disconnects → session ends → remote clients hide player UI

### Becoming the Audio Device

A client becomes the audio device when:
- **No session exists**: User plays any content → client auto-registers as audio device
- **Session exists**: User selects this device from the device selector in player UI → triggers playback transfer

### State Broadcasting

The audio device sends two types of state updates:

**1. Periodic state (every 5 seconds while playing):**
```json
{
  "type": "playback_state",
  "currentTrack": { "id": "track-123", "title": "...", "artist": "...", "album": "...", "duration": 240 },
  "queuePosition": 0,
  "queueVersion": 42,
  "position": 45.2,
  "isPlaying": true,
  "volume": 0.8,
  "shuffle": false,
  "repeat": "off",
  "timestamp": 1699999999999
}
```

**2. Queue sync (only when queue changes):**
```json
{
  "type": "queue_update",
  "queue": [{ "id": "track-123" }, { "id": "track-456" }],
  "queueVersion": 42
}
```

Remote clients track `queueVersion`. If received state has higher version than local, client sends `request_queue` message. Server responds with `queue_sync` containing the full queue from the audio device's last broadcast.

**Broadcast triggers:**
- **Periodic**: Every 5 seconds while playing (position sync)
- **On events**: Immediately on play/pause/seek/track-change (state update)
- **On queue change**: Immediately broadcast queue_update

**Position interpolation**: Remote clients should interpolate position locally between updates using `timestamp` to avoid jerky progress bars.

### Remote Commands

Remote clients can send commands:

| Command | Payload | Description |
|---------|---------|-------------|
| `play` | - | Resume playback |
| `pause` | - | Pause playback |
| `seek` | `{ position: number }` | Seek to position (seconds) |
| `next` | - | Skip to next track |
| `prev` | - | Go to previous track |
| `setVolume` | `{ volume: number }` | Set volume (0.0 - 1.0) |
| `setShuffle` | `{ enabled: boolean }` | Toggle shuffle mode |
| `setRepeat` | `{ mode: "off" \| "all" \| "one" }` | Set repeat mode |
| `addToQueue` | `{ trackId, position? }` | Add track to queue |
| `removeFromQueue` | `{ index }` | Remove track from queue |
| `clearQueue` | - | Clear the queue |
| `playTrack` | `{ trackId }` | Play specific track immediately |
| `becomeAudioDevice` | `{ transferId }` | Transfer audio playback to this client |

### Playback Transfer

When a remote client requests to become audio device (via device selector):

1. Client sends `becomeAudioDevice` with a unique `transferId`
2. Server generates transfer token, sends `prepare_transfer` to current audio device
3. Current audio device sends `transfer_ready` with final state + token, pauses playback
4. Server sends `become_audio_device` with state + token to new client
5. New client acknowledges with `transfer_complete`, begins playback
6. Server sends `transfer_complete` to old client, which fully stops
7. If any step times out (5 seconds), transfer is aborted and old device continues

This handshake prevents race conditions where both devices might play simultaneously.

### Reconnection

**Audio device reconnects within heartbeat window (15s):**
1. Client reconnects and sends `hello`
2. Server checks if this device was recently the audio device (by device fingerprint: name + type + user)
3. If match found and session still pending timeout:
   - Server cancels timeout
   - Sends `welcome` with `reclaimable: true`
   - Client sends `reclaim_audio_device` with last known state
   - Server validates state, restores audio device status
   - Session continues

```typescript
// Client → Server
{
  type: "reclaim_audio_device",
  state: PlaybackState  // current state from client
}
```

**Audio device reconnects after timeout:**
- Session already ended
- Client must start fresh (play content to become audio device)

**Remote client reconnects:**
- No special handling needed
- Receives current session state in `welcome` message

### Disconnection Handling

| Scenario | Behavior |
|----------|----------|
| Audio device disconnects cleanly | Session ends, remote clients hide player |
| Audio device stops responding | Server detects via heartbeat timeout (15s no state update) → broadcasts `playback_session_ended` |
| Remote client disconnects | No effect on session |
| All clients disconnect | Session ends (no persistence) |

**Heartbeat**: Server expects state updates from audio device at least every 15 seconds. If no update received, server assumes audio device crashed and ends the session.

### Volume

Volume in `PlaybackState` represents the **audio device's playback volume**, not device hardware volume.

**Behavior:**
- Audio device reports its player volume (0.0 - 1.0)
- Remote clients display this volume in their UI
- `setVolume` command changes the audio device's player volume
- Device hardware/system volume is NOT synced (user controls locally)

**Rationale:** Users expect to control their device's overall volume locally. Syncing application-level playback volume allows remote control without unexpected system volume changes.

## WebSocket Protocol

### Server → Client Messages

```typescript
// Welcome message (response to hello)
{
  type: "welcome",
  deviceId: string,
  session: {
    exists: boolean,
    state?: PlaybackState,
    queue?: QueueItem[],
    audioDeviceId?: string,
    reclaimable?: boolean
  },
  devices: ConnectedDevice[]
}

// Playback state update (from audio device, relayed to remotes)
{
  type: "playback_state",
  state: PlaybackState
}

// Full queue sync (response to request_queue)
{
  type: "queue_sync",
  queue: QueueItem[],
  queueVersion: number
}

// Session ended (audio device disconnected)
{
  type: "playback_session_ended"
}

// Command to audio device
{
  type: "playback_command",
  command: "play" | "pause" | "seek" | ...,
  payload?: object
}

// Device list changed (broadcast to all user's clients)
{
  type: "device_list_changed",
  devices: ConnectedDevice[],
  change: {
    type: "connected" | "disconnected" | "became_audio_device" | "stopped_audio_device",
    deviceId: string
  }
}

// Prepare to transfer playback (sent to current audio device)
{
  type: "prepare_transfer",
  transferId: string,
  targetDeviceId: string,
  targetDeviceName: string
}

// Become audio device (transfer playback to new device)
{
  type: "become_audio_device",
  transferId: string,
  state: PlaybackState,
  queue: QueueItem[]
}

// Transfer complete notification (sent to old audio device)
{
  type: "transfer_complete",
  transferId: string
}

// Transfer aborted (sent to both devices on timeout)
{
  type: "transfer_aborted",
  transferId: string,
  reason: "timeout" | "source_disconnected" | "target_disconnected"
}

// Error message
{
  type: "error",
  code: ErrorCode,
  message: string,
  context?: {
    command?: string,
    transferId?: string
  }
}
```

### Client → Server Messages

```typescript
// Hello message (sent on connect)
{
  type: "hello",
  deviceName: string,
  deviceType: "web" | "android" | "ios"
}

// Audio device broadcasting state
{
  type: "playback_state",
  state: PlaybackState
}

// Remote client sending command
{
  type: "playback_command",
  command: string,
  payload?: object
}

// Client registering as audio device
{
  type: "register_audio_device"
}

// Client unregistering (stopping playback)
{
  type: "unregister_audio_device"
}

// Request full queue (when version mismatch detected)
{
  type: "request_queue"
}

// Reclaim audio device status after reconnect
{
  type: "reclaim_audio_device",
  state: PlaybackState
}

// Current audio device ready to transfer (includes final state)
{
  type: "transfer_ready",
  transferId: string,
  state: PlaybackState,
  queue: QueueItem[]
}

// New audio device confirms transfer complete
{
  type: "transfer_complete",
  transferId: string
}
```

## Error Handling

The server sends error messages when operations fail:

```typescript
type ErrorCode =
  | "no_session"           // command sent but no active session
  | "not_audio_device"     // state broadcast from non-audio device
  | "command_failed"       // audio device couldn't execute command
  | "transfer_in_progress" // cannot start new transfer
  | "invalid_message"      // malformed message
  | "queue_limit_exceeded" // queue exceeds 500 tracks
```

**Error scenarios:**

| Scenario | Error Code | Behavior |
|----------|-----------|----------|
| Remote sends command, no session | `no_session` | Error returned to sender |
| Non-audio device broadcasts state | `not_audio_device` | Error returned, state ignored |
| Add to queue exceeds 500 limit | `queue_limit_exceeded` | Error returned, queue unchanged |
| Command to disconnected audio device | `no_session` | Session ends, error to sender |

## Data Types

```typescript
interface PlaybackState {
  currentTrack: Track | null;
  queuePosition: number;
  queueVersion: number;   // incremented on queue changes
  position: number;       // seconds
  isPlaying: boolean;
  volume: number;         // 0.0 - 1.0
  shuffle: boolean;
  repeat: "off" | "all" | "one";
  timestamp: number;      // for sync/interpolation
}

interface Track {
  id: string;
  title: string;
  artistId: string;
  artistName: string;
  albumId: string;
  albumTitle: string;
  duration: number;       // seconds
  trackNumber?: number;
  imageId?: string;       // for album art
}

interface QueueUpdate {
  queue: QueueItem[];     // max 500 items
  queueVersion: number;
}

interface QueueItem {
  id: string;             // track ID
  addedAt: number;        // timestamp
}

interface ConnectedDevice {
  id: string;             // unique connection ID
  name: string;           // auto-generated: "Chrome on Windows", "Android Phone"
  deviceType: "web" | "android" | "ios";
  isAudioDevice: boolean;
  connectedAt: number;    // timestamp
}
```

Note: `Track` is a minimal subset of the full catalog Track. Only fields needed for player UI are included to minimize payload size.

## Implementation Notes

### Server Changes
- Track connected devices per user (id, name, deviceType, isAudioDevice)
- Deduplicate device names per user (append " (2)", " (3)", etc. for collisions)
- Each WebSocket connection = one device, regardless of browser/app
- Track which client is the audio device for each user
- Route playback state messages to all other user clients
- Route commands to audio device
- Handle audio device registration/unregistration
- Implement transfer handshake protocol
- Heartbeat timeout detection (15s)
- Broadcast device list changes to all user clients
- Clean up on disconnect

### Client Changes (Web)
- Send device info on connect (browser, OS)
- Add "is audio device" state
- When audio device: broadcast state periodically (5s), broadcast queue on change
- When remote: display mirrored player UI, interpolate position, send commands
- Device selector dropdown in player UI showing all connected devices
- Handle incoming state updates, queue updates, and commands
- Handle transfer protocol (both as source and destination)

### Client Changes (Android)
- Same as web
- Send device info on connect (device model, Android version)
- Consider background service for audio device mode
- Handle audio focus properly when becoming/losing audio device
- Proper handling of audio interruptions (calls, other apps)

## Future Considerations

- Latency compensation for seek commands
- Buffering indicator for remote clients
- "Listeners" count display
- Playback quality/bitrate sync
- Offline queue (queue changes while briefly disconnected)
