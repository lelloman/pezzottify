# Pezzottify Android

Two independently installable Android apps:

| App | Gradle module | Application ID |
| --- | --- | --- |
| Pezzottify | `:app` (phone) | `com.lelloman.pezzottify.android` |
| Pezzottify-player | `:player-app` | `com.lelloman.pezzottify.android.player` |

Pezzottify streams from the server; Pezzottify-player opens and shares local audio
files and maintains local playlists without a server or login. The existing TV
variant remains `com.lelloman.pezzottify.android.tv`. Each APK has one launcher entry.
The `:player` module remains the streaming playback library; `:equalizer` and
`:theme` contain code shared by both apps.

The player has its own application, preferences, database, and URI grants. Local
playlists, playback state, and equalizer settings from the former bundled player
are not automatically migrated: reselect audio files and recreate playlists in
the new app. Updating Pezzottify preserves its existing package and streaming data.

## Overview

Pezzottify Android is a native Android music streaming application built with modern Android development practices and technologies. The application provides a full-featured music listening experience with catalog browsing, search, playback controls, and user content management.

**Technology Stack:**
- **Language**: Kotlin
- **UI Framework**: Jetpack Compose with Material 3
- **Dependency Injection**: Hilt (Dagger)
- **Networking**: Retrofit + OkHttp with Kotlin Serialization
- **Local Storage**: Room Database with encrypted preferences
- **Asynchronous Programming**: Kotlin Coroutines + Flow
- **Audio Playback**: ExoPlayer (Media3) with OkHttp integration
- **Architecture**: Clean Architecture with multi-module structure

The app is designed with clean architecture principles, separating concerns into distinct modules with well-defined dependencies. This modular approach ensures scalability, testability, and maintainability while allowing teams to work independently on different features.

## Features

- Browse music catalog (artists, albums, tracks)
- Audio playback with playback controls
- Search functionality with filters
- User authentication with multi-device support
- Playlist management
- Like/favorite content with cross-device sync
- Artist and album detail views
- Popular content discovery
- Listening history tracking
- Recently viewed content
- Search history
- Real-time sync via WebSocket
- Offline support with background synchronization
- Memory pressure management
- Debug interface for development

## Architecture

The project follows Clean Architecture principles with a multi-module Gradle setup. Dependencies flow inward from outer layers (UI, data sources) toward the core domain layer, ensuring business logic remains independent of frameworks and implementation details.

### Module Structure

#### Core Layers

**domain**
- Contains business logic, use cases, and domain models
- Defines interfaces for data sources (repositories, stores, API clients)
- Key components:
  - Use cases:
    - Authentication: `PerformLogin`, `PerformLogout`, `IsLoggedIn`
    - Content: `PerformSearch`, `GetPopularContent`, `InitializeApp`
    - User data: `LogViewedContentUseCase`, `GetRecentlyViewedContentUseCase`, `LogSearchHistoryEntryUseCase`, `GetSearchHistoryEntriesUseCase`
    - User content: `ToggleLikeUseCase`, `GetLikedStateUseCase`
    - Settings: `UpdateExternalSearchSetting`
  - Domain models: `Artist`, `Album`, `Track`, `AuthState`, `PopularContent`, `ViewedContent`, `LikedContent`, `ListeningEvent`
  - Store interfaces: `RemoteApiClient`, `StaticsStore`, `AuthStore`, `UserDataStore`, `UserContentStore`, `ConfigStore`, `ListeningEventStore`, `UserSettingsStore`, `SyncStateStore`, `PermissionsStore`
  - Player interfaces: `PezzottifyPlayer`, `PlatformPlayer`, `ControlsAndStatePlayer`
  - Sync system: `SyncManager`, `BaseSynchronizer`, `UserContentSynchronizer`, `ListeningEventSynchronizer`, `UserSettingsSynchronizer`, `StaticsSynchronizer`, `SyncWebSocketHandler`
  - Platform abstractions: `MemoryPressureMonitor`, `StorageMonitor`, `NetworkConnectivityObserver`, `AppLifecycleObserver`, `DeviceInfoProvider`
  - WebSocket: `WebSocketManager`, `WebSocketInitializer`
  - State management via Kotlin `StateFlow`

#### Data Layers

**remoteapi**
- Implements the `RemoteApiClient` interface from domain
- Handles all HTTP communication with the Pezzottify backend server
- Built with Retrofit, OkHttp, and Kotlin Serialization
- API endpoints:
  - Authentication: login, logout
  - Catalog: artists, albums, tracks, images, artist discography
  - Discovery: search with filters, popular content
  - User content: like/unlike content, get liked content
  - Listening: record listening events
  - Sync: get sync state, get sync events, update user settings
- Response models map server JSON to domain models
- Configurable host URL via `HostUrlProvider`

**localdata**
- Implements persistence interfaces from domain
- Uses Room Database for caching catalog data and user content
- Encrypted SharedPreferences for sensitive data (authentication tokens, config)
- Store implementations:
  - `AuthStoreImpl`: Authentication credentials and session state
  - `StaticsStoreImpl`: Catalog data caching
  - `StaticsItemFetchStateStoreImpl`: Fetch state tracking for data freshness
  - `UserDataStoreImpl`: Recently viewed content and search history
  - `UserContentStoreImpl`: Liked content with sync status
  - `ConfigStoreImpl`: App configuration
  - `ListeningEventStoreImpl`: Listening events pending sync
  - `UserSettingsStoreImpl`: Synced user settings
  - `SyncStateStoreImpl`: Synchronization state tracking
  - `PermissionsStoreImpl`: User permissions cache
- Three databases:
  - `StaticsDb`: Caches artists, albums, tracks, and discographies
  - `UserLocalDataDb`: Stores recently viewed content and search history
  - `UserContentDb`: Stores liked content, listening events, and sync metadata

**player**
- Implements platform-specific audio playback via `PlatformPlayer` interface
- `PlayerService`: Android foreground service for background playback
- `ExoPlatformPlayerModule`: ExoPlayer (Media3) integration with OkHttp for streaming
- `PlayerServiceEventsEmitter`: Broadcasts player state changes
- Handles audio streaming from the backend server with proper authentication

#### Presentation Layer

**ui**
- All Jetpack Compose UI components and screens
- Implements MVVM pattern with ViewModels and state hoisting
- Main screens:
  - `SplashScreen`: App initialization and loading
  - `LoginScreen`: User authentication
  - `MainScreen`: Bottom navigation container
  - `HomeScreen`: Recently viewed content and quick access
  - `SearchScreen`: Search interface with filters
  - `LibraryScreen`: User's music library
  - `ArtistScreen`, `AlbumScreen`, `TrackScreen`: Content detail views
  - `ProfileScreen`: User profile and settings
  - `SettingsScreen`, `StyleSettingsScreen`: App settings and style customization
  - `PlayerScreen`: Full-screen player view
  - `QueueScreen`: Playback queue management
  - `FullScreenImageScreen`: Full-screen image viewer
  - `AboutScreen`: App information
- Reusable components for content lists, cards, and player controls
- Material 3 theming and design system

**app**
- Main application module and entry point
- Hilt dependency injection setup (`@HiltAndroidApp`)
- Aggregates all feature modules
- Platform-specific implementations:
  - `AndroidMemoryPressureMonitor`: Responds to system memory pressure callbacks
  - `AndroidStorageMonitor`: Monitors device storage availability
  - `AndroidDeviceInfoProvider`: Provides device UUID and info for multi-device support
  - `AndroidNetworkConnectivityObserver`: Monitors network connectivity changes
  - `AndroidAppLifecycleObserver`: Tracks app foreground/background state
- DI modules: `ApplicationModule`, `DomainModule`, `UiModule`, `LifecycleModule`, `MemoryModule`, `StorageModule`, `InteractorsModule`
- `MainActivity` and `PezzottifyApplication` entry points
- Initializes the app via `InitializeApp` use case on startup

#### Supporting Modules

**logger**
- Centralized logging infrastructure
- Abstracts logging implementation for easier testing and configuration

**debuginterface**
- Debug tools and utilities for development builds only
- Included only in debug configurations (`debugImplementation`)
- Configuration overrides, network inspection, and debug controls

### Dependency Graph

```
app
├─ ui → logger
├─ domain → logger
├─ remoteapi → domain, logger
├─ localdata → domain, logger
├─ player → domain, logger
├─ logger
└─ debuginterface (debug only)
```

All data and player modules depend on the `domain` module for interface definitions. The `ui` module is framework-only (Compose) and depends on `logger` for logging. The `app` module ties everything together through dependency injection, providing concrete implementations to domain interfaces.

### Synchronization Architecture

The app implements a robust offline-first synchronization system:

1. **Local-first writes**: User actions (likes, settings changes) are saved locally with `PendingSync` status
2. **Background sync**: Synchronizers (`UserContentSynchronizer`, `ListeningEventSynchronizer`, `UserSettingsSynchronizer`) process pending items
3. **WebSocket real-time sync**: `SyncWebSocketHandler` receives server push notifications for changes from other devices
4. **Conflict resolution**: Server sequence numbers track sync state; full resync on sequence gaps
5. **Network awareness**: `NetworkConnectivityObserver` triggers sync when connectivity is restored
6. **Lifecycle awareness**: `AppLifecycleObserver` manages sync timing based on app state

## Development

### Building

Before building, prepare the pinned Androidoscopy SDK and session UI artifacts:

```bash
# From the repository root (requires the Android SDK/JDK):
bash scripts/prepare-androidoscopy.sh
```

This verifies the sibling `../androidoscopy` checkout against `androidoscopy.rev`
and publishes SDK/UI 2.0.2 to Maven Local. It never overwrites a dirty or different
checkout. Set `ANDROIDOSCOPY_CHECKOUT` to use another checkout. CI runs the same
preparation; the pinned commit must be available in Androidoscopy's remote before
CI can clone it. Separate builds are required because the projects use different AGP versions.

```bash
./gradlew build               # Build all modules
./gradlew :app:assemblePhoneDebug :player-app:assembleDebug
./gradlew :app:assemblePhoneRelease :player-app:assembleRelease
./gradlew :app:assembleTvDebug # Build the existing TV app
```

Debug APKs are written to `app/build/outputs/apk/phone/debug/app-phone-debug.apk`
and `player-app/build/outputs/apk/debug/player-app-debug.apk`. Both application
modules use the repository version and optional `signing.properties` release key.


### Equalizer

Phone and TV Settings expose a device-local ten-band equalizer (31, 62, 125, 250,
500 Hz, 1, 2, 4, 8, 16 kHz; ±12 dB in 0.5 dB steps). It defaults off. The current
curve, enabled state, and named profiles are persisted locally. When route observation
starts, the first known output loads its associated profile or the flat default. Profiles can
be loaded, saved, updated, renamed, and deleted. Editing a loaded profile changes
the current sound but never overwrites the saved curve without explicit Save.
Reset produces a flat custom curve; deleting a profile retains the current sound.
The compact band rows place frequency, slider, and gain side by side. A dedicated
**Profiles and audio outputs** screen manages profiles and their device associations.
Whenever the media output is known, each profile offers **Associate with current output: …**.
Associating loads the saved curve immediately, replacing current unsaved adjustments.
An output can have one assigned profile; a profile can be assigned to multiple outputs.
Removing an association keeps the current sound; deleting a profile also removes its associations.
Profiles and associations never sync to the server.

Routing observation starts with the application and continues for the process lifetime,
independently of playback and screen lifecycles. The actual Media3 AudioTrack route takes
precedence while playing. On Android 13+ the anticipated MEDIA/MUSIC route is queried
while paused or without an AudioTrack, using `AudioManager.getAudioDevicesForAttributes`.
Device callbacks trigger immediate refreshes; a one-second poll also catches selection
changes between already-connected devices. Merely connecting an unused device does not
select its profile. Earlier Android versions still need playback to identify the route.
A route change discards unsaved adjustments and loads the assigned saved profile, or
the default flat curve for any unassigned output, even if no associations exist at all.
Saved profiles are never modified by route changes. A temporarily unavailable route
does not reset the curve; the next known route is compared with the last known output.
The master EQ switch is never changed automatically. Duplicate route events, pauses,
and AudioTrack recreation on the same output do not overwrite manual adjustments.
Both catalog playback and the standalone player use this integration, with separate
preferences. The standalone player currently has no equalizer settings screen.

Bluetooth associations use a normalized device address, with name/alias for display;
on Android 12+ the UI requests Nearby devices (`BLUETOOTH_CONNECT`) permission.
No scan/location permission is needed. Speaker identity is fixed. Missing/redacted
addresses (including older Android versions without AudioDeviceInfo addresses),
unidentified analog headsets, and multiple simultaneous outputs cannot be associated.
The association button rechecks the current route before saving to prevent a stale UI
binding the wrong device. Android 13+ supports association even before starting playback.
Existing five-band curves and saved profiles migrate using logarithmic-frequency
interpolation, retaining names, selection, and enabled state. This approximates
the old settings; the response is not identical with the new band spacing.

PlaybackService inserts a PCM audio processor into Media3's sink. It uses peaking
biquads ([RBJ cookbook](https://www.w3.org/TR/audio-eq-cookbook/)), independent
channel history, 20 ms transitions, automatic headroom based on combined frequency
response, and saturating 16-bit output. Frequencies at/above Nyquist are skipped.
The processor remains in the chain when disabled to support live toggling;
disabled/flat output is an exact PCM bypass after any transition. Float output,
encoded passthrough and hardware offload are disabled to prevent bypassing the
equalizer, so high-resolution input is converted to 16-bit PCM by Media3.
This affects only playback on this Android device, not remotely controlled clients.

Tests cover persistence/profile isolation, invalid stored settings, frequency
response, stereo isolation, buffer/flush lifecycle, live toggling and UI actions.

### Diagnostic tools

Phone and TV Settings include a **Diagnostic session** button opening Androidoscopy's
pairing/start/stop screen. Debug builds retain the existing automatic dashboard;
release builds initialize inactive and require explicit activation, with a 15-minute
inactivity deadline. Both variants register the app-specific tools below; release
does not register legacy dashboard database/preferences/token actions. No session
activation is persisted in settings. Session expiry and process death are owned by
the SDK, and the SDK's non-exported UI handles pairing and notification permissions.

The card distinguishes off, waiting, pairing, connected and interrupted states
with icons, colors and text. The SDK session screen offers **Accept all**, off by
default and reset on session end. Enabling it requires a local confirmation:
any PC that can reach the session can then pair and use all tools without code
approval. Only one PC connects at a time; disabling the switch does not revoke an
already approved PC (Stop does). Automatically approved PCs are not remembered.

All names below have the `pezzottify_` prefix. Tools call the app's actual injected
managers/stores, not a separate diagnostic copy. Access requires an active, paired
Androidoscopy session. Stopping/expiring the session cancels SDK calls; actions
already completed are not rolled back. Calls have a 60-second timeout. Mutating
diagnostic calls are serialized with each other (not with ordinary app operations).

| Tool | Access |
| --- | --- |
| `playback` | Local player state, remote/local mode, error and paginated queue |
| `saved_playback` | Persisted playback and queue; **may delete expired/corrupt saved state**, matching the store's normal load behavior |
| `cache_status` | Database, memory and image sizes; memory hit/miss/eviction metrics |
| `sync_status` | Sync state, user/catalog cursors and full-sync requirement |
| `connectivity` | Network availability and WebSocket state/version/error |
| `catalog_item` | Local track/album/artist by `type` and `id`, with fetch state |
| `download_status` | Known download status/progress by content `id` |
| `playlist` | Local playlist by `id`, with paginated track IDs |
| `sync_backlog` | Pending playlists/listening events and loading catalog count |
| `logs` | Unredacted session logs, with cursor, level and exact-tag filters |
| `retry_playback` | **Mutating:** request player retry on the main thread |
| `sync_catch_up` | **Mutating:** run `user` or `catalog` catch-up; can update local data and user sync may fall back to full sync |
| `reconnect` | **Mutating:** disconnect/reconnect the server WebSocket |
| `cache_action` | **Mutating:** `trim`/`clear` the `statics` or `images` cache |

List tools accept `offset` (default 0) and `limit` (default/max 100). Pages return
`total`, `nextOffset` and `items`; these are live snapshots, not transactional
snapshots across calls. Backlog stores currently load pending collections before
the adapter paginates the response. Catalog reads do not fetch missing content.

`logs` captures calls through `LoggerFactory`, including Debug messages regardless
of ordinary log-level/file-logging settings. It does not capture arbitrary Android
logcat or read historical log files. Capture starts with the diagnostic session;
the in-memory buffer is cleared on stop/expiry/session replacement. It retains at
most 1000 entries, 4096 characters per message (including stack traces), marking
truncated entries. `after` defaults to 0, `limit` defaults to 100 (max 100),
`minimumLevel` accepts `Debug`, `Info`, `Warn`, `Error`, and `tag` is an exact match.
Responses also have a character budget to fit the transport frame, so a page can
contain fewer entries than requested. Poll with the returned `nextCursor`;
`oldestAvailableId` identifies the oldest retained entry and `hasMore` indicates
another page. Reading logs is session
activity, but log production itself does not extend the inactivity deadline.

Logs are deliberately **not sanitized**: URLs, bodies, credentials and personal
data already written by app code may be visible to the paired PC. There is no
arbitrary reflection, raw SQL write tool, auth-store dump or token-refresh tool in
the release tool set. Use only with a trusted PC.

### Testing

```bash
./gradlew test                # Run all unit tests
./run-integration-tests.sh    # Run integration tests (requires Docker)
```

**Unit tests** are located in each module's `src/test/` directory and test individual components in isolation.

**Integration tests** are located in `remoteapi/src/integrationTest/` and test the remote API client against a real backend server. The `run-integration-tests.sh` script:
- Creates a test catalog with sample data (artist, album, track, image)
- Builds and runs a pezzottify-server Docker container
- Creates a test database with authentication credentials
- Runs the integration test suite
- Cleans up all resources automatically

Integration tests require Docker to be installed and running.

### Running

```bash
./gradlew :app:installPhoneDebug
./gradlew :player-app:installDebug
```

The app can be launched from the device or via:
```bash
adb shell am start -n com.lelloman.pezzottify.android/.MainActivity
```

## Backend

The streaming app requires the Pezzottify server to be running. See the main project README for instructions on setting up the backend server.

Default server URL can be configured in the app's settings or debug interface.

## License

See the main project LICENSE file for details.

## Shared assistant development

The Rust-backed assistant is built from the sibling `simple-android-assistant` checkout
(or `-PassistantCheckout=/path/to/checkout`) until the matching release is published.
From the repository root, run `bash scripts/checkout-simple-android-assistant.sh`
to provision the revision pinned in `simple-android-assistant.rev`. The script
leaves existing checkouts unchanged and requires them to match the pin and be clean.
Android CI provisions this checkout, all four Android Rust targets, NDK
27.0.12077973, and cargo-ndk 4.1.2 before running lint and unit tests.
See that repository's README for the Rust/NDK prerequisites. Existing Room history
is imported by the version 2 database migration.
