# Android Development Improvements TODO

This document tracks development-related improvements for correctness, reliability, performance, and best practices.

---

## Critical Priority

### 1. Replace GlobalScope Usage

**Problem:** 12 files use `GlobalScope`, causing coroutine lifecycle management issues, potential memory leaks, and testing difficulties.

**Affected files:**
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/internal/PlayerImpl.kt:35`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:51`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventSynchronizer.kt:55`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/user/LogViewedContentUseCase.kt:22`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/usercontent/ToggleLikeUseCase.kt:26`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/usercontent/UserContentSynchronizer.kt:42`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/UserSettingsSynchronizer.kt:51`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/StaticsSynchronizer.kt:59`
- [ ] `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt:57`
- [ ] `player/src/main/java/com/lelloman/pezzottify/android/player/ExoPlatformPlayer.kt:39`
- [ ] `player/src/main/java/com/lelloman/pezzottify/android/player/PlayerServiceEventsEmitter.kt:14`

**Solution:**
1. Create an `@ApplicationScope` qualifier and provide an application-scoped `CoroutineScope` via Hilt
2. Inject this scope instead of defaulting to `GlobalScope`
3. Ensure proper cancellation on app termination

---

### 2. Remove runBlocking() from Main Thread Startup

**Problem:** `AuthStoreImpl.initialize()` uses `runBlocking` and is called from `Application.onCreate()`, blocking the main thread during app startup. This can cause ANRs on slow devices.

**Location:** `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:66-88`

**Solution options:**
- [ ] Use AndroidX Startup `Initializer` with async initialization
- [ ] Convert to a suspend function and call from a coroutine in Application
- [ ] Use a `CompletableDeferred` pattern for initialization state

---

## Medium Priority

### 3. Standardize JVM Target Across Modules

**Problem:** Inconsistent JVM targets can cause compatibility issues and is confusing for maintenance.

| Module | Current JVM Target |
|--------|-------------------|
| app | 1.8 |
| ui | 1.8 |
| player | 1.8 |
| domain | 11 |
| remoteapi | 11 |
| localdata | 11 |
| logger | 11 |
| debuginterface | 11 |

**Solution:**
- [ ] Update `app/build.gradle.kts` to JVM 11
- [ ] Update `ui/build.gradle.kts` to JVM 11
- [ ] Update `player/build.gradle.kts` to JVM 11
- [ ] Verify no compatibility issues after change

---

### 4. Enable R8/ProGuard for Release Builds

**Problem:** All modules have `isMinifyEnabled = false` in release builds, resulting in larger APK size, no code obfuscation, and unused code not being stripped.

**Location:** `app/build.gradle.kts:53`

**Solution:**
- [ ] Enable `isMinifyEnabled = true` for release builds
- [ ] Create/update ProGuard rules for Retrofit, Kotlin Serialization, Room, Hilt
- [ ] Test release build thoroughly
- [ ] Consider enabling `shrinkResources = true`

---

### 5. Replace Not-Null Assertions (!!) with Safe Alternatives

**Problem:** Not-null assertions can throw `NullPointerException` at runtime.

**Locations:**
- [ ] `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt:100` - `body()!!`
- [ ] `player/src/main/java/com/lelloman/pezzottify/android/player/ExoPlatformPlayer.kt:184` - `sessionToken!!`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:215` - `savedEventId!!`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:216` - `savedEventId!!`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:235` - `savedEventId!!`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:236` - `savedEventId!!`
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/response/ActivityPeriod.kt:52` - JSON parsing
- [ ] `domain/src/main/java/com/lelloman/pezzottify/android/domain/remoteapi/response/ActivityPeriod.kt:57` - JSON parsing

**Solution:** Use `?.let`, `requireNotNull()` with descriptive message, or proper null handling with early returns.

---

### 6. Fix Inconsistent Logging Practices

**Problem:** Direct `Log.e()` and `printStackTrace()` usage instead of the injected logger.

**Locations:**
- [ ] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:61` - uses `Log.e()`
- [ ] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:80` - uses `e.printStackTrace()`

**Solution:** Inject and use the logger from the logger module consistently.

---

## Low Priority

### 7. Improve Test Coverage

**Current state:** 46 test files for 356 production files (~13% file coverage)

**Areas needing more tests:**
- [ ] `localdata` module - database operations, stores
- [ ] `player` module - playback logic
- [ ] `remoteapi` module - API client edge cases
- [ ] Integration tests for cross-module flows

---

### 8. Implement or Remove Unimplemented Methods

**Problem:** Methods throwing `TODO("Not yet implemented")` will crash if called.

**Location:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/internal/PlayerImpl.kt`

**Methods:**
- [ ] `canGoToPreviousPlaylist` (line 46)
- [ ] `canGoToNextPlaylist` (line 49)
- [ ] `goToPreviousPlaylist()` (line 214)
- [ ] `goToNextPlaylist()` (line 218)
- [ ] `moveTrack()` (line 222)

**Solution:** Either implement the functionality or throw `UnsupportedOperationException` with a descriptive message (or remove from interface if not needed).

---

### 9. Replace Broad Exception Catching with Specific Types

**Problem:** Generic `catch (e: Exception)` blocks make debugging harder and may hide issues.

**Locations to review:**
- [ ] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:79`
- [ ] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/config/ConfigStoreImpl.kt:45`
- [ ] Other locations catching generic Exception/Throwable

**Solution:** Catch specific exception types and handle appropriately, or at minimum log the exception type for debugging.

---

### 10. Reduce Code Duplication in PlayerImpl

**Problem:** `loadAlbum()`, `loadUserPlaylist()`, and `loadSingleTrack()` have similar structure.

**Location:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/internal/PlayerImpl.kt`

**Solution:**
- [ ] Extract common playlist loading logic into a private helper method
- [ ] Parameterize the context type and track ID resolution

---

### 11. Migrate from kapt to KSP

**Problem:** `kapt` (Kotlin Annotation Processing Tool) is slower than KSP (Kotlin Symbol Processing).

**Current usage:** All modules use `kotlin.kapt` plugin for Hilt annotation processing.

**Solution:**
- [ ] Verify Hilt KSP support for current Hilt version
- [ ] Replace `kapt` with `ksp` in all build.gradle.kts files
- [ ] Update Hilt dependencies to use KSP variant
- [ ] Test build and runtime behavior

---

### 12. Fix Home Screen Flash of Empty State

**Problem:** When opening the app, the home screen briefly shows the "empty" state (e.g., "Start exploring...") for a fraction of a second before content loads.

**Cause:** Likely a race condition where the UI renders before data is fetched, or the initial state defaults to empty instead of loading.

**Solution:**
- [ ] Investigate HomeScreenViewModel initial state
- [ ] Ensure initial state is "Loading" rather than "Empty"
- [ ] Consider showing a loading indicator or skeleton UI until data is ready

---

## Summary

| Priority | Count | Items |
|----------|-------|-------|
| Critical | 1 | GlobalScope |
| Medium | 4 | JVM targets, R8, !!, logging |
| Low | 6 | Tests, TODOs, exceptions, duplication, KSP, home screen flash |

---

## Notes

- The CLAUDE.md already documents the "Don't use GlobalScope" rule - this needs enforcement
- Consider adding lint rules or static analysis to prevent regression on these issues
- Some items (like KSP migration) may require dependency version updates
