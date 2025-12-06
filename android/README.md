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
- **Audio Playback**: Custom player implementation with platform-specific components
- **Architecture**: Clean Architecture with multi-module structure

The app is designed with clean architecture principles, separating concerns into distinct modules with well-defined dependencies. This modular approach ensures scalability, testability, and maintainability while allowing teams to work independently on different features.

## Features

- Browse music catalog (artists, albums, tracks)
- Audio playback with playback controls
- Search functionality
- User authentication
- Playlist management
- Like/favorite content
- Artist and album detail views
- Debug interface for development

## Architecture

The project follows Clean Architecture principles with a multi-module Gradle setup. Dependencies flow inward from outer layers (UI, data sources) toward the core domain layer, ensuring business logic remains independent of frameworks and implementation details.

### Module Structure

#### Core Layers

**domain**
- Contains business logic, use cases, and domain models
- Defines interfaces for data sources (repositories, stores, API clients)
- Framework-agnostic and has no Android dependencies
- Key components:
  - Use cases: `PerformLogin`, `PerformLogout`, `PerformSearch`, `InitializeApp`, `IsLoggedIn`
  - Domain models: `Artist`, `Album`, `Track`, `AuthState`
  - Interfaces: `RemoteApiClient`, `StaticsStore`, `AuthStore`, `PezzottifyPlayer`
  - State management via Kotlin `StateFlow`

#### Data Layers

**remoteapi**
- Implements the `RemoteApiClient` interface from domain
- Handles all HTTP communication with the Pezzottify backend server
- Built with Retrofit, OkHttp, and Kotlin Serialization
- API endpoints for login, catalog browsing (artists, albums, tracks), search, and image fetching
- Response models map server JSON to domain models
- Configurable host URL via `HostUrlProvider`

**localdata**
- Implements persistence interfaces from domain (`AuthStore`, `StaticsStore`, `UserDataStore`)
- Uses Room Database for caching catalog data and user content
- Encrypted SharedPreferences for sensitive data (authentication tokens, config)
- Two databases:
  - `StaticsDb`: Caches artists, albums, tracks, and discographies
  - `UserDataDb`: Stores recently viewed content
- Fetch state tracking to manage data freshness and avoid redundant network calls

**player**
- Implements platform-specific audio playback
- Integrates with the `PezzottifyPlayer` interface from domain
- Handles audio streaming from the backend server

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
- Minimal code - primarily DI configuration and `MainActivity`
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
├─ ui
├─ domain
├─ remoteapi → domain
├─ localdata → domain
├─ player → domain
├─ logger
└─ debuginterface (debug only)
```

All feature modules depend on the `domain` module, ensuring business logic remains centralized and testable. The `app` module ties everything together through dependency injection.

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
