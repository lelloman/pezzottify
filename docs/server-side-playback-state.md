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

### Remote Client
- A connected client that is NOT the audio device
- Displays the player UI mirroring the audio device's state
- Can send control commands (play, pause, seek, skip, etc.)
- Shows device selector to switch audio output to any connected device
- Has full access to queue (synced on queue changes)

### Playback Session
- Exists as long as an audio device is connected
- Contains: current track, queue, position, play/pause state, volume
- Cleared when audio device disconnects (no persistence)
- One session per user (single user = single session)
- **Queue limit**: 500 tracks maximum (hard limit, ~30KB payload)

## Behaviors

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

Remote clients track `queueVersion`. If received state has higher version than local, client requests full queue sync.

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
| `addToQueue` | `{ trackId, position? }` | Add track to queue |
| `removeFromQueue` | `{ index }` | Remove track from queue |
| `clearQueue` | - | Clear the queue |
| `playTrack` | `{ trackId }` | Play specific track immediately |
| `becomeAudioDevice` | - | Transfer audio playback to this client |

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

### Disconnection Handling

| Scenario | Behavior |
|----------|----------|
| Audio device disconnects cleanly | Session ends, remote clients hide player |
| Audio device stops responding | Server detects via heartbeat timeout (15s no state update) → broadcasts `playback_session_ended` |
| Remote client disconnects | No effect on session |
| All clients disconnect | Session ends (no persistence) |

**Heartbeat**: Server expects state updates from audio device at least every 15 seconds. If no update received, server assumes audio device crashed and ends the session.

## WebSocket Protocol

### Server → Client Messages

```typescript
// Playback state update (from audio device, relayed to remotes)
{
  type: "playback_state",
  state: PlaybackState
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

// Become audio device (transfer playback)
{
  type: "become_audio_device",
  state: PlaybackState
}
```

### Client → Server Messages

```typescript
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
```

## Data Types

```typescript
interface PlaybackState {
  currentTrack: Track | null;
  queuePosition: number;
  queueVersion: number;   // incremented on queue changes
  position: number;       // seconds
  isPlaying: boolean;
  volume: number;         // 0.0 - 1.0
  timestamp: number;      // for sync/interpolation
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
  isAudioDevice: boolean;
}
```

## Implementation Notes

### Server Changes
- Track connected devices per user (id, name, isAudioDevice)
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
