# CLAUDE.md - Android Project

This file provides guidance to Claude Code when working with the Pezzottify Android application.

## Project Overview

Pezzottify Android is a native music streaming application built with modern Android development practices. The app follows Clean Architecture principles with a multi-module Gradle setup, ensuring separation of concerns and testability.

**Technology Stack:**
- **Language**: Kotlin
- **UI Framework**: Jetpack Compose with Material 3
- **Dependency Injection**: Hilt (Dagger)
- **Networking**: Retrofit + OkHttp with Kotlin Serialization
- **Local Storage**: Room Database + Encrypted SharedPreferences
- **Asynchronous Programming**: Kotlin Coroutines + Flow
- **Architecture**: Clean Architecture with MVVM pattern

**Build Configuration:**
- Minimum SDK: 24 (Android 7.0)
- Target/Compile SDK: 36
- JVM Target: 1.8 (app), 11 (domain and some modules)
- Application ID: `com.lelloman.pezzottify.android`

## Development Commands

### Building

```bash
cd android
./gradlew build                    # Build all modules
./gradlew assembleDebug            # Build debug APK
./gradlew assembleRelease          # Build release APK
./gradlew clean                    # Clean build artifacts
```

### Testing

```bash
./gradlew test                     # Run all unit tests
./gradlew testDebugUnitTest        # Run debug unit tests
./gradlew connectedAndroidTest     # Run instrumented tests (requires device/emulator)
./gradlew :{module}:test           # Run tests for specific module
./run-integration-tests.sh         # Run integration tests (requires Docker)
```

**Integration tests** are located in `remoteapi/src/integrationTest/` and test the remote API client against a real catalog-server instance. The `run-integration-tests.sh` script handles all setup (Docker container, test database, sample catalog) and cleanup automatically. Integration tests are kept separate from unit tests to maintain fast test execution with `./gradlew test`.

### Running

```bash
./gradlew installDebug             # Install debug APK on connected device
./gradlew installDebug && adb shell am start -n com.lelloman.pezzottify.android/.MainActivity  # Install and launch
```

### Code Quality

```bash
./gradlew lint                     # Run Android lint checks
./gradlew lintDebug                # Run lint on debug variant
```

### Dependencies

```bash
./gradlew dependencies             # View dependency tree for all modules
./gradlew app:dependencies         # View dependencies for app module
```

## Module Structure

The project uses a multi-module architecture with clear separation of concerns:

### Core Modules

#### domain
- **Purpose**: Business logic, use cases, and domain models
- **Key principle**: Framework-agnostic, no Android dependencies
- **Location**: `/android/domain/`
- **Package**: `com.lelloman.pezzottify.android.domain`

**Key Components:**
- **Use Cases**: Business logic operations
  - `PerformLogin`: Handle user authentication
  - `PerformSearch`: Search catalog content
  - `InitializeApp`: App startup logic
  - `LoadArtistScreen`, `LoadAlbumScreen`, `LoadTrackScreen`: Content loading
  - `SyncStatics`: Synchronize catalog data from server
- **Domain Models**: Core data structures
  - `Artist`, `Album`, `Track`: Content models
  - `AuthState`: Authentication state management
  - `AppEvent`: Application-wide events
- **Interfaces**: Contracts for data sources
  - `RemoteApiClient`: HTTP API contract
  - `StaticsStore`: Catalog data persistence contract
  - `AuthStore`: Authentication storage contract
  - `UserDataStore`: User content storage contract
  - `PezzottifyPlayer`: Audio playback contract
- **State Management**: Uses Kotlin `StateFlow` for reactive state
- **Package Structure**:
  - `auth/`: Authentication domain logic
  - `user/`: User-related domain logic
  - `player/`: Playback domain logic
  - `statics/`: Static catalog data logic
  - `cache/`: In-memory caching (LruCache, StaticsCache, CacheMetricsCollector)
  - `memory/`: Memory pressure monitoring (MemoryPressureMonitor interface)
  - `remoteapi/`: API contracts and response models
  - `usecase/`: Use case implementations
  - `config/`: Configuration models
  - `sync/`: Data synchronization logic

**In-Memory Cache System:**
- `StaticsCache`: LRU cache for Artist, Album, Track objects with 5-minute TTL
- `MemoryPressureMonitor`: Interface for detecting memory pressure levels (LOW/MEDIUM/HIGH/CRITICAL)
- `AndroidMemoryPressureMonitor`: Platform implementation in app module using ComponentCallbacks2
- Cache sizes dynamically adjust based on memory pressure (10MB at LOW to 512KB at CRITICAL)
- User can toggle cache on/off in Profile > Performance settings
- Cache is cleared on logout
- `CacheMetricsCollector`: Tracks hit rates and latency for performance analysis

**Testing**: Unit tests use JUnit, Truth assertions, and Coroutines Test

### Data Layer Modules

#### remoteapi
- **Purpose**: HTTP communication with backend server
- **Dependencies**: domain, logger
- **Location**: `/android/remoteapi/`
- **Package**: `com.lelloman.pezzottify.android.remoteapi`

**Key Components:**
- Implements `RemoteApiClient` interface from domain
- Built with Retrofit, OkHttp, and Kotlin Serialization
- `RetrofitRemoteApiClient`: Main implementation
- `HostUrlProvider`: Configurable backend URL
- Endpoint groups:
  - Authentication: Login/logout
  - Catalog: Artists, albums, tracks, images
  - Search: Content search
  - User content: Playlists, likes

**Response Models**: Map JSON responses to domain models

#### localdata
- **Purpose**: Local persistence (database + encrypted preferences)
- **Dependencies**: domain, logger
- **Location**: `/android/localdata/`
- **Package**: `com.lelloman.pezzottify.android.localdata`

**Key Components:**
- **Databases**:
  - `StaticsDb`: Caches artists, albums, tracks, discographies (Room)
  - `UserDataDb`: Stores recently viewed content (Room)
- **Encrypted Storage**: Sensitive data (auth tokens, config) in EncryptedSharedPreferences
- **Fetch State Tracking**: Manages data freshness to avoid redundant network calls
- Implements domain interfaces:
  - `StaticsStore`: Catalog data persistence
  - `AuthStore`: Authentication storage
  - `UserDataStore`: User content storage

**Important Patterns:**
- Uses Room DAOs for database operations
- All database operations run in coroutines (IO dispatcher)
- Foreign key constraints ensure referential integrity
- Fetch states track last sync times per entity

#### player
- **Purpose**: Audio playback implementation
- **Dependencies**: domain, logger
- **Location**: `/android/player/`
- **Package**: `com.lelloman.pezzottify.android.player`

**Key Components:**
- Implements `PezzottifyPlayer` interface from domain
- Handles audio streaming from backend
- Manages playback state (playing, paused, stopped)
- Track queue management

**Note**: Also see `platform-player` module for platform-specific implementations

### Presentation Layer

#### ui
- **Purpose**: All UI components and screens (Jetpack Compose)
- **Dependencies**: domain, logger
- **Location**: `/android/ui/`
- **Package**: `com.lelloman.pezzottify.android.ui`

**Architecture Pattern**: MVVM
- **ViewModels**: Handle business logic and state
- **Screens**: Compose UI functions
- **State Classes**: Immutable state representations
- **Events**: User interactions
- **Actions**: Navigation and side effects

**Main Screens:**
- `SplashScreen`: App initialization and loading
- `LoginScreen`: User authentication
- `MainScreen`: Bottom navigation container
- `HomeScreen`: Recently viewed content and quick access
- `SearchScreen`: Search interface with filters
- `LibraryScreen`: User's music library
- `ArtistScreen`: Artist details and discography
- `AlbumScreen`: Album details and tracks
- `TrackScreen`: Track details
- `ProfileScreen`: User profile and settings
- `AboutScreen`: App information

**Reusable Components:**
- Content lists and cards
- Player controls
- Navigation components
- Loading states and error handling
- Material 3 themed components

**Package Structure:**
- `screen/`: Screen composables and ViewModels
- `content/`: Content display components
- `navigation/`: Navigation logic
- `theme/`: Material 3 theming

### Application Module

#### app
- **Purpose**: Main application module and entry point
- **Dependencies**: All feature modules (ui, domain, remoteapi, localdata, player, logger, debuginterface)
- **Location**: `/android/app/`
- **Package**: `com.lelloman.pezzottify.android`

**Key Components:**
- `PezzottifyApplication`: Application class with `@HiltAndroidApp`
- `MainActivity`: Single activity with Compose UI
- **Dependency Injection Modules**:
  - `ApplicationModule`: Application-level dependencies
  - `DomainModule`: Domain layer bindings

**Responsibilities:**
- Hilt DI setup and module aggregation
- App lifecycle management
- Initializes app via `InitializeApp` use case on startup
- Minimal code - primarily DI configuration

### Supporting Modules

#### logger
- **Purpose**: Centralized logging infrastructure
- **Location**: `/android/logger/`
- **Package**: `com.lelloman.pezzottify.android.logger`

**Key Components:**
- Abstracts logging implementation
- Makes testing easier (can mock logger)
- Provides consistent logging interface across modules

#### debuginterface
- **Purpose**: Debug tools and utilities
- **Location**: `/android/debuginterface/`
- **Package**: `com.lelloman.pezzottify.android.debuginterface`

**Important**: Only included in debug builds (`debugImplementation` in app module)

**Features:**
- Configuration overrides
- Network inspection tools
- Debug controls and settings
- Development utilities

#### platform-player
- **Purpose**: Platform-specific player implementations
- **Location**: `/android/platform-player/`
- **Note**: Listed in settings.gradle.kts but not currently connected in dependency graph

## Architecture Patterns

### Clean Architecture

Dependencies flow inward from outer layers to core:
```
UI Layer (ui, app)
    ↓
Domain Layer (domain)
    ↑
Data Layer (remoteapi, localdata, player)
```

**Key Principles:**
- Domain layer has NO dependencies on other layers
- Data layer depends only on domain layer
- UI layer depends on domain layer (and indirectly data layer via DI)
- Business logic lives in domain (use cases)
- Framework details isolated in outer layers

### Dependency Injection (Hilt)

**Setup:**
- `@HiltAndroidApp` on `PezzottifyApplication`
- `@AndroidEntryPoint` on `MainActivity` and ViewModels
- Modules in app module provide dependencies
- Constructor injection preferred over field injection

**Common Patterns:**
```kotlin
// ViewModels
@HiltViewModel
class MyViewModel @Inject constructor(
    private val useCase: MyUseCase
) : ViewModel()

// Providing dependencies
@Module
@InstallIn(SingletonComponent::class)
object MyModule {
    @Provides
    @Singleton
    fun provideMyDependency(): MyDependency = MyDependencyImpl()
}
```

### State Management

**Domain Layer:**
- Uses `StateFlow` for reactive state
- Use cases expose state via StateFlow
- Immutable state objects

**UI Layer:**
- ViewModels expose state via StateFlow
- Screens collect state with `collectAsState()`
- Unidirectional data flow: Events → ViewModel → State → UI

**Example:**
```kotlin
// ViewModel
class MyViewModel @Inject constructor() : ViewModel() {
    private val _state = MutableStateFlow(MyState())
    val state: StateFlow<MyState> = _state.asStateFlow()

    fun onEvent(event: MyEvent) {
        // Handle event, update state
    }
}

// Screen
@Composable
fun MyScreen(viewModel: MyViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsState()
    // Render UI based on state
}
```

### Coroutines and Flow

**Dispatchers:**
- `Dispatchers.IO`: Network calls, database operations
- `Dispatchers.Main`: UI updates
- `Dispatchers.Default`: CPU-intensive work

**Flow Patterns:**
- Repository methods return Flow for reactive data
- Use `flowOn()` to specify dispatcher
- Use `stateIn()` to convert Flow to StateFlow
- Collect flows in ViewModels within `viewModelScope`

## Key Conventions

### Package Structure
- Group by feature/layer, not by type
- Domain models at package root
- Interfaces define contracts
- Internal implementations in `internal/` subpackage if needed

### Naming Conventions
- **Use Cases**: Verb-first names (`PerformLogin`, `LoadArtistScreen`)
- **ViewModels**: `{Screen}ViewModel` (`LoginViewModel`)
- **Screens**: `{Screen}Screen` (`LoginScreen`)
- **State Classes**: `{Screen}State` (`LoginScreenState`)
- **Events**: `{Screen}Event` or `{Screen}Events`
- **Database Entities**: Match domain models with `Entity` suffix if different

### File Organization
- One top-level class per file
- File name matches class name
- Keep related classes in same package
- Group related functionality in subpackages

### Compose Conventions
- Composables are `PascalCase` functions
- Preview functions annotated with `@Preview`
- State hoisting: lift state to lowest common ancestor
- Remember stateful objects with `remember`
- Side effects in `LaunchedEffect` or `DisposableEffect`

### Testing Conventions
- Unit test files in `src/test/java/`
- Instrumented test files in `src/androidTest/java/`
- Test class name: `{ClassUnderTest}Test`
- Use Truth assertions: `assertThat(actual).isEqualTo(expected)`
- Mock dependencies with appropriate framework

## Common Tasks

### Adding a New Screen

1. Create screen state class in `ui/screen/{feature}/{Screen}State.kt`
2. Create events/actions classes if needed
3. Create ViewModel in `ui/screen/{feature}/{Screen}ViewModel.kt` with `@HiltViewModel`
4. Create Compose screen in `ui/screen/{feature}/{Screen}Screen.kt`
5. Add navigation route and destination
6. Wire up in navigation graph

### Adding a New Use Case

1. Create use case in `domain/usecase/{UseCase}.kt`
2. Define interface dependencies (if any)
3. Implement business logic
4. Inject into ViewModel
5. Add unit tests in `domain/src/test/`

### Adding a New API Endpoint

1. Add endpoint method to Retrofit interface in `remoteapi/`
2. Create response model if needed
3. Map response to domain model
4. Update `RemoteApiClient` interface in domain (if new contract method)
5. Call from use case or repository

### Adding a New Database Entity

1. Create entity class with `@Entity` in `localdata/`
2. Create DAO interface with `@Dao`
3. Add to database class `@Database(entities = [...])`
4. Implement store interface from domain
5. Provide via Hilt module
6. Consider database migration if modifying existing schema

### Adding Dependencies

1. Add to version catalog in `gradle/libs.versions.toml` (if using catalog)
2. Or add directly to module's `build.gradle.kts` in `dependencies` block
3. Sync Gradle files
4. Import and use in code

### Running on Device/Emulator

1. Connect device or start emulator
2. Verify connection: `adb devices`
3. Build and install: `./gradlew installDebug`
4. Launch from device or via: `adb shell am start -n com.lelloman.pezzottify.android/.MainActivity`

## Backend Connection

The app requires the Pezzottify catalog server to be running:

1. Start backend server (see main repository CLAUDE.md or catalog-server README)
2. Default server URL: `http://10.0.2.2:3001` (Android emulator localhost)
3. Configure server URL in app:
   - Via debug interface (debug builds only)
   - Via settings screen
   - Or modify `HostUrlProvider` implementation

**Emulator Network:**
- `10.0.2.2` maps to `127.0.0.1` on host machine
- Use actual IP address for physical devices on same network

## Debugging

### Common Issues

**Hilt Errors:**
- Ensure `@HiltAndroidApp` on Application class
- Ensure `@AndroidEntryPoint` on Activity/ViewModel
- Check module installation targets match component hierarchy
- Rebuild project after DI changes

**Compose UI Not Updating:**
- Ensure state is `State<T>` or collected with `collectAsState()`
- Check if state object is immutable (data class)
- Verify state updates happen on Main dispatcher

**Database Errors:**
- Check schema version matches
- Implement migrations for schema changes
- Clear app data if schema changed without migration
- Verify foreign key constraints

**Network Errors:**
- Verify backend server is running
- Check server URL configuration
- Verify network permissions in manifest
- Check HTTP vs HTTPS
- For emulator: use `10.0.2.2` for localhost

### Debug Tools

**Android Studio:**
- Logcat for logs
- Layout Inspector for Compose UI hierarchy
- Database Inspector for Room databases
- Network Inspector for HTTP traffic
- Profiler for performance analysis

**Debug Interface Module:**
- Access via debug builds only
- Override configurations
- Inspect network requests
- Test different scenarios

### Logging

Use logger module for consistent logging:
```kotlin
@Inject lateinit var logger: Logger

logger.d("Debug message")
logger.i("Info message")
logger.w("Warning message")
logger.e("Error message", throwable)
```

## Testing Guidelines

### Unit Tests (domain and ViewModels)

**Location**: `{module}/src/test/java/`

**Setup:**
- Use JUnit 4
- Truth for assertions
- Kotlinx Coroutines Test for testing coroutines
- MockK or Mockito for mocking

**Key Patterns:**
```kotlin
@Test
fun `test description in backticks`() = runTest {
    // Given
    val useCase = MyUseCase()

    // When
    val result = useCase.execute()

    // Then
    assertThat(result).isEqualTo(expected)
}
```

**Testing Coroutines:**
```kotlin
@Test
fun `test with coroutines`() = runTest {
    val flow = flowOf(1, 2, 3)
    val result = flow.toList()
    assertThat(result).containsExactly(1, 2, 3)
}
```

### Instrumented Tests (UI and Database)

**Location**: `{module}/src/androidTest/java/`

**Setup:**
- Requires device or emulator
- Use AndroidJUnit4 runner
- Espresso for UI testing (or Compose test tools)
- Truth for assertions

**Compose UI Testing:**
```kotlin
@Test
fun testScreen() {
    composeTestRule.setContent {
        MyScreen()
    }

    composeTestRule.onNodeWithText("Expected Text").assertIsDisplayed()
    composeTestRule.onNodeWithTag("button").performClick()
}
```

## Important Notes

### Do's
- Follow Clean Architecture boundaries
- Keep domain layer framework-agnostic
- Use dependency injection (constructor injection)
- Write immutable data classes for state
- Use StateFlow for reactive state
- Test use cases and ViewModels
- Use Kotlin Coroutines for async operations
- Follow Material 3 design guidelines in UI

### Don'ts
- Don't add Android dependencies to domain module
- Don't bypass use cases and call repositories directly from ViewModels
- Don't use mutable state in UI layer
- Don't perform network/database operations on Main thread
- Don't use `GlobalScope` for coroutines (use `viewModelScope` or proper scope)
- Don't create tight coupling between modules
- Don't bypass the DI framework with manual instantiation

### Performance Considerations
- Use `remember` in Compose to avoid recomposition
- Use `LazyColumn` for long lists
- Use `derivedStateOf` for computed state
- Paginate large data sets
- Cache images and data appropriately
- Profile before optimizing

## Related Documentation

- Main repository CLAUDE.md (root level) for overall project guidance
- README.md in this directory for project overview
- TODO.md in root directory for known issues and future work
- Catalog server documentation in `catalog-server/README.md`

## Questions or Issues

For Android-specific development questions or issues:
1. Check this CLAUDE.md file
2. Review README.md in android directory
3. Check module-specific build.gradle.kts files
4. Review TODO.md for known limitations
5. Consult official Android and Jetpack Compose documentation
