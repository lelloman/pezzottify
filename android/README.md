# Pezzottify Android

Android client for the Pezzottify music streaming platform.

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
    - Settings: `UpdateDirectDownloadsSetting`
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

```bash
./gradlew build               # Build all modules
./gradlew assembleDebug       # Build debug APK
./gradlew assembleRelease     # Build release APK
```

### Testing

```bash
./gradlew test                # Run all unit tests
./run-integration-tests.sh    # Run integration tests (requires Docker)
```

**Unit tests** are located in each module's `src/test/` directory and test individual components in isolation.

**Integration tests** are located in `remoteapi/src/integrationTest/` and test the remote API client against a real backend server. The `run-integration-tests.sh` script:
- Creates a test catalog with sample data (artist, album, track, image)
- Builds and runs a catalog-server Docker container
- Creates a test database with authentication credentials
- Runs the integration test suite
- Cleans up all resources automatically

Integration tests require Docker to be installed and running.

### Running

```bash
./gradlew installDebug        # Install debug APK on connected device/emulator
```

The app can be launched from the device or via:
```bash
adb shell am start -n com.lelloman.pezzottify.android/.MainActivity
```

## Backend

The Android app requires the Pezzottify catalog server to be running. See the main project README for instructions on setting up the backend server.

Default server URL can be configured in the app's settings or debug interface.

## License

See the main project LICENSE file for details.
