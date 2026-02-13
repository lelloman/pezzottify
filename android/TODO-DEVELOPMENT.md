# Android Development Improvements TODO

This document tracks development-related improvements for correctness, reliability, performance, and best practices.

---

## Critical Priority

### 1. Replace GlobalScope Usage ✅ COMPLETED

**Problem:** 12 files use `GlobalScope`, causing coroutine lifecycle management issues, potential memory leaks, and testing difficulties.

**Affected files:**
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/internal/PlayerImpl.kt:35`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:51`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningEventSynchronizer.kt:55`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/user/LogViewedContentUseCase.kt:22`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/usercontent/ToggleLikeUseCase.kt:26`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/usercontent/UserContentSynchronizer.kt:42`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/UserSettingsSynchronizer.kt:51`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/StaticsSynchronizer.kt:59`
- [x] `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt:57`
- [x] `player/src/main/java/com/lelloman/pezzottify/android/player/ExoPlatformPlayer.kt:39`
- [x] `player/src/main/java/com/lelloman/pezzottify/android/player/PlayerServiceEventsEmitter.kt:14`

**Solution:**
- [x] Created `@ApplicationScope` qualifier and provided an application-scoped `CoroutineScope` via Hilt
- [x] Injected this scope instead of defaulting to `GlobalScope`
- [x] Proper cancellation on app termination

**Commit:** `[android] Replace GlobalScope with application-scoped CoroutineScope`

---

### 2. Remove runBlocking() from Main Thread Startup ✅ ALREADY FIXED

**Problem:** `AuthStoreImpl.initialize()` was thought to use `runBlocking` but it was already fixed.

**Status:** Already using `coroutineScope.launch` which is non-blocking.

---

## Medium Priority

### 3. Standardize JVM Target Across Modules ✅ COMPLETED

**Problem:** Inconsistent JVM targets can cause compatibility issues and is confusing for maintenance.

| Module | Current JVM Target |
|--------|-------------------|
| app | 11 ✅ |
| ui | 11 ✅ |
| player | 11 ✅ |
| domain | 11 |
| remoteapi | 11 |
| localdata | 11 |
| logger | 11 |
| debuginterface | 11 |

**Solution:**
- [x] Updated `app/build.gradle.kts` to JVM 11
- [x] Updated `ui/build.gradle.kts` to JVM 11
- [x] Updated `player/build.gradle.kts` to JVM 11
- [x] Verified no compatibility issues after change

**Commit:** `[android] Standardize JVM target to version 11`

---

### 4. Enable R8/ProGuard for Release Builds ✅ COMPLETED

**Problem:** All modules have `isMinifyEnabled = false` in release builds, resulting in larger APK size, no code obfuscation, and unused code not being stripped.

**Location:** `app/build.gradle.kts:53`

**Solution:**
- [x] Enable `isMinifyEnabled = true` for release builds
- [x] Create/update ProGuard rules for Retrofit, Kotlin Serialization, Room, Hilt
- [x] Test release build thoroughly
- [ ] Consider enabling `shrinkResources = true` (blocked by AGP 9.0 DSL changes)

**Commit:** `[android] Enable R8/ProGuard for release builds`

---

### 5. Replace Not-Null Assertions (!!) with Safe Alternatives ✅ COMPLETED

**Problem:** Not-null assertions can throw `NullPointerException` at runtime.

**Locations:**
- [x] `remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImpl.kt:100` - `body()!!`
- [x] `player/src/main/java/com/lelloman/lezzottify/android/player/ExoPlatformPlayer.kt:184` - `sessionToken!!`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:215` - `savedEventId!!`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:216` - `savedEventId!!`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:235` - `savedEventId!!`
- [x] `domain/src/main/java/com/lelloman/pezzottify/android/domain/listening/ListeningTracker.kt:236` - `savedEventId!!`
- Additional cases found and fixed in `LocalExoPlayer.kt`, `PlaybackMetadataProviderImpl.kt`, `RemotePlaybackMetadataProvider.kt`, `ChatRepositoryImpl.kt`

**Solution:** Used `?.let`, local variables, and proper null handling instead of `!!`.

**Commit:** `[android] Replace not-null assertions and unimplemented method crashes`

---

### 6. Fix Inconsistent Logging Practices ✅ COMPLETED

**Problem:** Direct `Log.e()` and `printStackTrace()` usage instead of the injected logger.

**Locations:**
- [x] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:61` - uses `Log.e()`
- [x] `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/internal/auth/AuthStoreImpl.kt:80` - uses `e.printStackTrace()`

**Solution:** Injected and used the logger from the logger module consistently.

**Commit:** `[android] Fix inconsistent logging in AuthStoreImpl`

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

### 8. Implement or Remove Unimplemented Methods ✅ COMPLETED

**Problem:** Methods throwing `TODO("Not yet implemented")` will crash if called.

**Location:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/player/internal/PlayerImpl.kt`

**Methods:**
- [x] `canGoToPreviousPlaylist` (line 46) - Always false, feature not implemented
- [x] `canGoToNextPlaylist` (line 49) - Always false, feature not implemented
- [x] `goToPreviousPlaylist()` (line 214) - Now logs warning instead of crashing
- [x] `goToNextPlaylist()` (line 218) - Now logs warning instead of crashing
- [x] `moveTrack()` (line 222) - Now logs warning instead of crashing

**Solution:** Replaced `TODO("Not yet implemented")` with `logger.warn()` calls. These methods are part of the public API and `moveTrack()` is actively used in the UI (QueueScreen undo functionality), so they must not crash. Full implementation is deferred.

**Commit:** `[android] Replace not-null assertions and unimplemented method crashes`

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

---

## Summary

| Priority | Count | Items |
|----------|-------|-------|
| Critical | 2 | GlobalScope ✅, runBlocking ✅ |
| Medium | 4 | JVM targets ✅, R8 ✅, !! ✅, logging ✅ |
| Low | 6 | Tests, TODOs ✅, exceptions, duplication, KSP, home screen flash |

---

## Notes

- The CLAUDE.md already documents the "Don't use GlobalScope" rule - this needs enforcement
- Consider adding lint rules or static analysis to prevent regression on these issues
- Some items (like KSP migration) may require dependency version updates
