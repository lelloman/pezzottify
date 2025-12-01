# Implementation Plan: In-Memory Cache Layer for Content Resolution

## Overview

This plan covers the implementation of:
1. **Memory Pressure Component** - detects available memory for dynamic cache sizing
2. **In-Memory Cache Layer** - LRU cache with size-based eviction and TTL expiration
3. **Performance Metrics** - validates cache effectiveness vs Room

Target: Cache `Artist`, `Album`, and `Track` objects in `StaticsProvider` for faster access than Room database queries.

---

## Phase 1: Memory Pressure Component

### 1.1 Create Domain Interface

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/memory/MemoryPressureMonitor.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.memory

import kotlinx.coroutines.flow.StateFlow

enum class MemoryPressureLevel {
    LOW,      // Plenty of memory available
    MEDIUM,   // Moderate memory available
    HIGH,     // Low memory, reduce cache
    CRITICAL  // Very low memory, minimize cache
}

data class MemoryInfo(
    val availableBytes: Long,
    val maxHeapBytes: Long,
    val usedBytes: Long,
    val pressureLevel: MemoryPressureLevel
)

interface MemoryPressureMonitor {
    /**
     * Current memory info as a StateFlow for reactive updates
     */
    val memoryInfo: StateFlow<MemoryInfo>

    /**
     * Get recommended cache size in bytes based on current memory pressure
     */
    fun getRecommendedCacheSizeBytes(): Long

    /**
     * Get recommended max entries for a specific item type
     */
    fun getRecommendedMaxEntries(itemType: CacheItemType): Int

    /**
     * Force a memory info refresh
     */
    fun refresh()
}

enum class CacheItemType {
    ARTIST,
    ALBUM,
    TRACK
}
```

### 1.2 Platform Implementation

**File:** `app/src/main/java/com/lelloman/pezzottify/android/memory/AndroidMemoryPressureMonitor.kt`

```kotlin
package com.lelloman.pezzottify.android.memory

import android.app.ActivityManager
import android.app.Application
import android.content.ComponentCallbacks2
import android.content.res.Configuration
import com.lelloman.pezzottify.android.domain.memory.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidMemoryPressureMonitor @Inject constructor(
    private val application: Application,
    private val activityManager: ActivityManager
) : MemoryPressureMonitor, ComponentCallbacks2 {

    private val _memoryInfo = MutableStateFlow(calculateMemoryInfo())
    override val memoryInfo: StateFlow<MemoryInfo> = _memoryInfo.asStateFlow()

    // Configurable thresholds (percentage of max heap)
    private val lowThreshold = 0.70      // <70% used = LOW pressure
    private val mediumThreshold = 0.80   // 70-80% used = MEDIUM
    private val highThreshold = 0.90     // 80-90% used = HIGH
    // >90% = CRITICAL

    // Base cache sizes per pressure level (in entries)
    private val cacheSizes = mapOf(
        MemoryPressureLevel.LOW to CacheSizeConfig(
            artistEntries = 200,
            albumEntries = 300,
            trackEntries = 500,
            totalBytes = 10 * 1024 * 1024  // 10MB
        ),
        MemoryPressureLevel.MEDIUM to CacheSizeConfig(
            artistEntries = 100,
            albumEntries = 150,
            trackEntries = 250,
            totalBytes = 5 * 1024 * 1024   // 5MB
        ),
        MemoryPressureLevel.HIGH to CacheSizeConfig(
            artistEntries = 50,
            albumEntries = 75,
            trackEntries = 100,
            totalBytes = 2 * 1024 * 1024   // 2MB
        ),
        MemoryPressureLevel.CRITICAL to CacheSizeConfig(
            artistEntries = 20,
            albumEntries = 30,
            trackEntries = 50,
            totalBytes = 512 * 1024        // 512KB
        )
    )

    init {
        application.registerComponentCallbacks(this)
    }

    private fun calculateMemoryInfo(): MemoryInfo {
        val runtime = Runtime.getRuntime()
        val maxHeap = runtime.maxMemory()
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        val available = maxHeap - usedMemory

        val usageRatio = usedMemory.toDouble() / maxHeap.toDouble()
        val level = when {
            usageRatio < lowThreshold -> MemoryPressureLevel.LOW
            usageRatio < mediumThreshold -> MemoryPressureLevel.MEDIUM
            usageRatio < highThreshold -> MemoryPressureLevel.HIGH
            else -> MemoryPressureLevel.CRITICAL
        }

        return MemoryInfo(
            availableBytes = available,
            maxHeapBytes = maxHeap,
            usedBytes = usedMemory,
            pressureLevel = level
        )
    }

    override fun getRecommendedCacheSizeBytes(): Long {
        return cacheSizes[_memoryInfo.value.pressureLevel]?.totalBytes ?: 0
    }

    override fun getRecommendedMaxEntries(itemType: CacheItemType): Int {
        val config = cacheSizes[_memoryInfo.value.pressureLevel] ?: return 0
        return when (itemType) {
            CacheItemType.ARTIST -> config.artistEntries
            CacheItemType.ALBUM -> config.albumEntries
            CacheItemType.TRACK -> config.trackEntries
        }
    }

    override fun refresh() {
        _memoryInfo.value = calculateMemoryInfo()
    }

    // ComponentCallbacks2 implementation for system memory pressure events
    override fun onTrimMemory(level: Int) {
        refresh()
        // Could also trigger cache eviction here for aggressive cleanup
    }

    override fun onConfigurationChanged(newConfig: Configuration) {}
    override fun onLowMemory() {
        _memoryInfo.value = _memoryInfo.value.copy(
            pressureLevel = MemoryPressureLevel.CRITICAL
        )
    }

    private data class CacheSizeConfig(
        val artistEntries: Int,
        val albumEntries: Int,
        val trackEntries: Int,
        val totalBytes: Long
    )
}
```

### 1.3 Hilt Module

**File:** `app/src/main/java/com/lelloman/pezzottify/android/di/MemoryModule.kt`

```kotlin
@Module
@InstallIn(SingletonComponent::class)
object MemoryModule {

    @Provides
    @Singleton
    fun provideActivityManager(application: Application): ActivityManager {
        return application.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
    }

    @Provides
    @Singleton
    fun provideMemoryPressureMonitor(
        application: Application,
        activityManager: ActivityManager
    ): MemoryPressureMonitor {
        return AndroidMemoryPressureMonitor(application, activityManager)
    }
}
```

---

## Phase 2: In-Memory Cache Implementation

### 2.1 Cache Entry Model

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/cache/CacheEntry.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.cache

data class CacheEntry<T>(
    val value: T,
    val createdAt: Long,
    val lastAccessedAt: Long,
    val sizeBytes: Int
) {
    fun isExpired(ttlMillis: Long, currentTime: Long): Boolean {
        return (currentTime - createdAt) > ttlMillis
    }

    fun touch(currentTime: Long): CacheEntry<T> {
        return copy(lastAccessedAt = currentTime)
    }
}
```

### 2.2 Generic LRU Cache with Size Eviction

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/cache/LruCache.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.cache

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

class LruCache<K, V>(
    private val maxEntries: () -> Int,
    private val maxSizeBytes: () -> Long,
    private val ttlMillis: Long,
    private val sizeCalculator: (V) -> Int,
    private val timeProvider: () -> Long = { System.currentTimeMillis() }
) {
    private val cache = ConcurrentHashMap<K, CacheEntry<V>>()
    private val lock = ReentrantReadWriteLock()

    // Metrics
    private var hits = 0L
    private var misses = 0L
    private var evictions = 0L
    private var expirations = 0L

    fun get(key: K): V? = lock.read {
        val entry = cache[key] ?: run {
            misses++
            return null
        }

        val now = timeProvider()
        if (entry.isExpired(ttlMillis, now)) {
            cache.remove(key)
            expirations++
            misses++
            return null
        }

        // Update last accessed time (LRU tracking)
        cache[key] = entry.touch(now)
        hits++
        return entry.value
    }

    fun put(key: K, value: V) = lock.write {
        val now = timeProvider()
        val size = sizeCalculator(value)

        // Remove expired entries first
        evictExpired(now)

        // Evict if needed for size constraints
        evictIfNeeded(size)

        cache[key] = CacheEntry(
            value = value,
            createdAt = now,
            lastAccessedAt = now,
            sizeBytes = size
        )
    }

    fun remove(key: K) = lock.write {
        cache.remove(key)
    }

    fun clear() = lock.write {
        cache.clear()
    }

    fun getMetrics(): CacheMetrics {
        return CacheMetrics(
            hits = hits,
            misses = misses,
            evictions = evictions,
            expirations = expirations,
            currentEntries = cache.size,
            currentSizeBytes = cache.values.sumOf { it.sizeBytes.toLong() },
            hitRate = if (hits + misses > 0) hits.toDouble() / (hits + misses) else 0.0
        )
    }

    fun resetMetrics() {
        hits = 0
        misses = 0
        evictions = 0
        expirations = 0
    }

    private fun evictExpired(now: Long) {
        val expired = cache.entries.filter { it.value.isExpired(ttlMillis, now) }
        expired.forEach {
            cache.remove(it.key)
            expirations++
        }
    }

    private fun evictIfNeeded(incomingSize: Int) {
        val maxEntriesNow = maxEntries()
        val maxBytesNow = maxSizeBytes()

        // Evict by entry count
        while (cache.size >= maxEntriesNow && cache.isNotEmpty()) {
            evictLru()
        }

        // Evict by size
        var currentSize = cache.values.sumOf { it.sizeBytes.toLong() }
        while (currentSize + incomingSize > maxBytesNow && cache.isNotEmpty()) {
            evictLru()
            currentSize = cache.values.sumOf { it.sizeBytes.toLong() }
        }
    }

    private fun evictLru() {
        val lruKey = cache.entries
            .minByOrNull { it.value.lastAccessedAt }
            ?.key

        if (lruKey != null) {
            cache.remove(lruKey)
            evictions++
        }
    }
}

data class CacheMetrics(
    val hits: Long,
    val misses: Long,
    val evictions: Long,
    val expirations: Long,
    val currentEntries: Int,
    val currentSizeBytes: Long,
    val hitRate: Double
)
```

### 2.3 Statics Cache

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/cache/StaticsCache.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.cache

import com.lelloman.pezzottify.android.domain.memory.CacheItemType
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Track
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class StaticsCache @Inject constructor(
    private val memoryPressureMonitor: MemoryPressureMonitor
) {
    companion object {
        // TTL: 5 minutes (content doesn't change frequently)
        private const val TTL_MILLIS = 5 * 60 * 1000L

        // Estimated sizes (adjust based on actual measurements)
        private const val ARTIST_SIZE_BYTES = 512   // ~0.5KB per artist
        private const val ALBUM_SIZE_BYTES = 1024   // ~1KB per album
        private const val TRACK_SIZE_BYTES = 768    // ~0.75KB per track
    }

    val artistCache = LruCache<String, Artist>(
        maxEntries = { memoryPressureMonitor.getRecommendedMaxEntries(CacheItemType.ARTIST) },
        maxSizeBytes = { memoryPressureMonitor.getRecommendedCacheSizeBytes() / 3 },
        ttlMillis = TTL_MILLIS,
        sizeCalculator = { ARTIST_SIZE_BYTES }
    )

    val albumCache = LruCache<String, Album>(
        maxEntries = { memoryPressureMonitor.getRecommendedMaxEntries(CacheItemType.ALBUM) },
        maxSizeBytes = { memoryPressureMonitor.getRecommendedCacheSizeBytes() / 3 },
        ttlMillis = TTL_MILLIS,
        sizeCalculator = { ALBUM_SIZE_BYTES }
    )

    val trackCache = LruCache<String, Track>(
        maxEntries = { memoryPressureMonitor.getRecommendedMaxEntries(CacheItemType.TRACK) },
        maxSizeBytes = { memoryPressureMonitor.getRecommendedCacheSizeBytes() / 3 },
        ttlMillis = TTL_MILLIS,
        sizeCalculator = { TRACK_SIZE_BYTES }
    )

    fun clearAll() {
        artistCache.clear()
        albumCache.clear()
        trackCache.clear()
    }

    fun getAllMetrics(): Map<String, CacheMetrics> {
        return mapOf(
            "artist" to artistCache.getMetrics(),
            "album" to albumCache.getMetrics(),
            "track" to trackCache.getMetrics()
        )
    }
}
```

---

## Phase 3: Integration with StaticsProvider

### 3.1 Modify StaticsProvider

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/statics/StaticsProvider.kt`

Add caching layer to existing implementation:

```kotlin
class StaticsProvider @Inject constructor(
    private val staticsStore: StaticsStore,
    private val staticItemFetchStateStore: StaticItemFetchStateStore,
    private val staticsSynchronizer: StaticsSynchronizer,
    private val staticsCache: StaticsCache,  // NEW
    private val metricsCollector: CacheMetricsCollector  // NEW
) {

    fun provideArtist(itemId: String): Flow<StaticsItem<Artist>> {
        // Check in-memory cache first
        staticsCache.artistCache.get(itemId)?.let { cached ->
            metricsCollector.recordCacheHit("artist")
            return flowOf(StaticsItem.Loaded(itemId, cached))
        }
        metricsCollector.recordCacheMiss("artist")

        // Fall back to database flow
        return staticsStore.getArtist(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Artist)) { artist, fetchState ->
                // Cache successful loads
                artist?.let {
                    staticsCache.artistCache.put(itemId, it)
                }

                resolveStaticsItem(itemId, artist, fetchState, StaticItemType.Artist)
            }
    }

    fun provideAlbum(itemId: String): Flow<StaticsItem<Album>> {
        staticsCache.albumCache.get(itemId)?.let { cached ->
            metricsCollector.recordCacheHit("album")
            return flowOf(StaticsItem.Loaded(itemId, cached))
        }
        metricsCollector.recordCacheMiss("album")

        return staticsStore.getAlbum(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Album)) { album, fetchState ->
                album?.let { staticsCache.albumCache.put(itemId, it) }
                resolveStaticsItem(itemId, album, fetchState, StaticItemType.Album)
            }
    }

    fun provideTrack(itemId: String): Flow<StaticsItem<Track>> {
        staticsCache.trackCache.get(itemId)?.let { cached ->
            metricsCollector.recordCacheHit("track")
            return flowOf(StaticsItem.Loaded(itemId, cached))
        }
        metricsCollector.recordCacheMiss("track")

        return staticsStore.getTrack(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Track)) { track, fetchState ->
                track?.let { staticsCache.trackCache.put(itemId, it) }
                resolveStaticsItem(itemId, track, fetchState, StaticItemType.Track)
            }
    }

    // Clear cache on logout
    fun clearCache() {
        staticsCache.clearAll()
    }

    private fun <T> resolveStaticsItem(
        itemId: String,
        item: T?,
        fetchState: StaticItemFetchState?,
        itemType: StaticItemType
    ): StaticsItem<T> {
        // Existing logic unchanged
        return when {
            item != null -> StaticsItem.Loaded(itemId, item)
            fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
            fetchState?.errorReason != null -> {
                maybeScheduleRetry(itemId, itemType, fetchState)
                StaticsItem.Error(itemId, fetchState.errorReason)
            }
            else -> {
                scheduleItemFetch(itemId, itemType)
                StaticsItem.Loading(itemId)
            }
        }
    }
}
```

---

## Phase 4: User Setting for Cache Toggle

### 4.1 Settings Interface in Domain

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/settings/AppSettings.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.settings

import kotlinx.coroutines.flow.StateFlow

interface AppSettings {
    /**
     * Whether the in-memory cache is enabled.
     * When disabled, all requests go directly to Room database.
     */
    val isInMemoryCacheEnabled: StateFlow<Boolean>

    suspend fun setInMemoryCacheEnabled(enabled: Boolean)
}
```

### 4.2 Settings Implementation (Persisted)

**File:** `localdata/src/main/java/com/lelloman/pezzottify/android/localdata/settings/AppSettingsImpl.kt`

```kotlin
package com.lelloman.pezzottify.android.localdata.settings

import android.content.SharedPreferences
import androidx.core.content.edit
import com.lelloman.pezzottify.android.domain.settings.AppSettings
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AppSettingsImpl @Inject constructor(
    private val sharedPreferences: SharedPreferences
) : AppSettings {

    companion object {
        private const val KEY_IN_MEMORY_CACHE_ENABLED = "in_memory_cache_enabled"
        private const val DEFAULT_CACHE_ENABLED = true  // Enabled by default
    }

    private val _isInMemoryCacheEnabled = MutableStateFlow(
        sharedPreferences.getBoolean(KEY_IN_MEMORY_CACHE_ENABLED, DEFAULT_CACHE_ENABLED)
    )
    override val isInMemoryCacheEnabled: StateFlow<Boolean> = _isInMemoryCacheEnabled.asStateFlow()

    override suspend fun setInMemoryCacheEnabled(enabled: Boolean) {
        sharedPreferences.edit {
            putBoolean(KEY_IN_MEMORY_CACHE_ENABLED, enabled)
        }
        _isInMemoryCacheEnabled.value = enabled
    }
}
```

### 4.3 Update StaticsProvider to Respect Setting

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/statics/StaticsProvider.kt`

```kotlin
class StaticsProvider @Inject constructor(
    private val staticsStore: StaticsStore,
    private val staticItemFetchStateStore: StaticItemFetchStateStore,
    private val staticsSynchronizer: StaticsSynchronizer,
    private val staticsCache: StaticsCache,
    private val metricsCollector: CacheMetricsCollector,
    private val appSettings: AppSettings  // NEW
) {

    fun provideArtist(itemId: String): Flow<StaticsItem<Artist>> {
        // Check if cache is enabled AND item is in cache
        if (appSettings.isInMemoryCacheEnabled.value) {
            staticsCache.artistCache.get(itemId)?.let { cached ->
                metricsCollector.recordCacheHit("artist")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            metricsCollector.recordCacheMiss("artist")
        }

        // Fall back to database flow
        return staticsStore.getArtist(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Artist)) { artist, fetchState ->
                // Only cache if setting is enabled
                if (appSettings.isInMemoryCacheEnabled.value) {
                    artist?.let { staticsCache.artistCache.put(itemId, it) }
                }
                resolveStaticsItem(itemId, artist, fetchState, StaticItemType.Artist)
            }
    }

    fun provideAlbum(itemId: String): Flow<StaticsItem<Album>> {
        if (appSettings.isInMemoryCacheEnabled.value) {
            staticsCache.albumCache.get(itemId)?.let { cached ->
                metricsCollector.recordCacheHit("album")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            metricsCollector.recordCacheMiss("album")
        }

        return staticsStore.getAlbum(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Album)) { album, fetchState ->
                if (appSettings.isInMemoryCacheEnabled.value) {
                    album?.let { staticsCache.albumCache.put(itemId, it) }
                }
                resolveStaticsItem(itemId, album, fetchState, StaticItemType.Album)
            }
    }

    fun provideTrack(itemId: String): Flow<StaticsItem<Track>> {
        if (appSettings.isInMemoryCacheEnabled.value) {
            staticsCache.trackCache.get(itemId)?.let { cached ->
                metricsCollector.recordCacheHit("track")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            metricsCollector.recordCacheMiss("track")
        }

        return staticsStore.getTrack(itemId)
            .combine(staticItemFetchStateStore.get(itemId, StaticItemType.Track)) { track, fetchState ->
                if (appSettings.isInMemoryCacheEnabled.value) {
                    track?.let { staticsCache.trackCache.put(itemId, it) }
                }
                resolveStaticsItem(itemId, track, fetchState, StaticItemType.Track)
            }
    }

    // ... rest of the class unchanged
}
```

### 4.4 Settings UI Component

**File:** `ui/src/main/java/com/lelloman/pezzottify/android/ui/screen/profile/CacheSettingsSection.kt`

```kotlin
package com.lelloman.pezzottify.android.ui.screen.profile

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun CacheSettingsSection(
    isCacheEnabled: Boolean,
    onCacheEnabledChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier.padding(16.dp)) {
        Text(
            text = "Performance",
            style = MaterialTheme.typography.titleMedium
        )

        Spacer(modifier = Modifier.height(8.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "In-memory cache",
                    style = MaterialTheme.typography.bodyLarge
                )
                Text(
                    text = "Cache content in memory for faster loading. Disable if experiencing issues.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Switch(
                checked = isCacheEnabled,
                onCheckedChange = onCacheEnabledChanged
            )
        }
    }
}
```

### 4.5 Integration with Profile/Settings Screen

Update the existing profile or settings screen to include the cache toggle:

```kotlin
// In ProfileScreenViewModel or SettingsScreenViewModel
@HiltViewModel
class ProfileScreenViewModel @Inject constructor(
    private val appSettings: AppSettings,
    // ... other dependencies
) : ViewModel() {

    val isCacheEnabled: StateFlow<Boolean> = appSettings.isInMemoryCacheEnabled

    fun onCacheEnabledChanged(enabled: Boolean) {
        viewModelScope.launch {
            appSettings.setInMemoryCacheEnabled(enabled)
            // Optionally clear cache when disabled
            if (!enabled) {
                staticsCache.clearAll()
            }
        }
    }
}

// In ProfileScreen composable
@Composable
fun ProfileScreen(viewModel: ProfileScreenViewModel = hiltViewModel()) {
    val isCacheEnabled by viewModel.isCacheEnabled.collectAsState()

    // ... other profile content

    CacheSettingsSection(
        isCacheEnabled = isCacheEnabled,
        onCacheEnabledChanged = viewModel::onCacheEnabledChanged
    )
}
```

### 4.6 Hilt Module for Settings

**File:** `app/src/main/java/com/lelloman/pezzottify/android/di/SettingsModule.kt`

```kotlin
@Module
@InstallIn(SingletonComponent::class)
object SettingsModule {

    @Provides
    @Singleton
    fun provideAppSettings(
        sharedPreferences: SharedPreferences
    ): AppSettings {
        return AppSettingsImpl(sharedPreferences)
    }
}
```

---

## Phase 5: Performance Metrics

### 5.1 Metrics Collector Interface

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/metrics/CacheMetricsCollector.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.metrics

interface CacheMetricsCollector {
    fun recordCacheHit(cacheType: String)
    fun recordCacheMiss(cacheType: String)
    fun recordCacheLatency(cacheType: String, latencyMs: Long)
    fun recordDbLatency(cacheType: String, latencyMs: Long)
    fun getReport(): CachePerformanceReport
}

data class CachePerformanceReport(
    val cacheHits: Map<String, Long>,
    val cacheMisses: Map<String, Long>,
    val avgCacheLatencyMs: Map<String, Double>,
    val avgDbLatencyMs: Map<String, Double>,
    val hitRates: Map<String, Double>,
    val estimatedTimeSavedMs: Long
)
```

### 5.2 Metrics Implementation

**File:** `domain/src/main/java/com/lelloman/pezzottify/android/domain/metrics/CacheMetricsCollectorImpl.kt`

```kotlin
package com.lelloman.pezzottify.android.domain.metrics

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class CacheMetricsCollectorImpl @Inject constructor() : CacheMetricsCollector {

    private val hits = ConcurrentHashMap<String, AtomicLong>()
    private val misses = ConcurrentHashMap<String, AtomicLong>()
    private val cacheLatencies = ConcurrentHashMap<String, MutableList<Long>>()
    private val dbLatencies = ConcurrentHashMap<String, MutableList<Long>>()

    override fun recordCacheHit(cacheType: String) {
        hits.computeIfAbsent(cacheType) { AtomicLong() }.incrementAndGet()
    }

    override fun recordCacheMiss(cacheType: String) {
        misses.computeIfAbsent(cacheType) { AtomicLong() }.incrementAndGet()
    }

    override fun recordCacheLatency(cacheType: String, latencyMs: Long) {
        cacheLatencies.computeIfAbsent(cacheType) { mutableListOf() }.add(latencyMs)
    }

    override fun recordDbLatency(cacheType: String, latencyMs: Long) {
        dbLatencies.computeIfAbsent(cacheType) { mutableListOf() }.add(latencyMs)
    }

    override fun getReport(): CachePerformanceReport {
        val hitCounts = hits.mapValues { it.value.get() }
        val missCounts = misses.mapValues { it.value.get() }

        val hitRates = hitCounts.keys.associateWith { key ->
            val h = hitCounts[key] ?: 0
            val m = missCounts[key] ?: 0
            if (h + m > 0) h.toDouble() / (h + m) else 0.0
        }

        val avgCacheLatency = cacheLatencies.mapValues { (_, latencies) ->
            if (latencies.isNotEmpty()) latencies.average() else 0.0
        }

        val avgDbLatency = dbLatencies.mapValues { (_, latencies) ->
            if (latencies.isNotEmpty()) latencies.average() else 0.0
        }

        // Estimate time saved: (cache hits) * (avg db latency - avg cache latency)
        val timeSaved = hitCounts.entries.sumOf { (key, count) ->
            val dbAvg = avgDbLatency[key] ?: 0.0
            val cacheAvg = avgCacheLatency[key] ?: 0.0
            (count * (dbAvg - cacheAvg)).toLong()
        }

        return CachePerformanceReport(
            cacheHits = hitCounts,
            cacheMisses = missCounts,
            avgCacheLatencyMs = avgCacheLatency,
            avgDbLatencyMs = avgDbLatency,
            hitRates = hitRates,
            estimatedTimeSavedMs = timeSaved
        )
    }
}
```

### 5.3 Debug Interface for Metrics

**File:** `debuginterface/src/main/java/com/lelloman/pezzottify/android/debuginterface/CacheDebugPanel.kt`

```kotlin
// Add to debug interface for viewing metrics in debug builds
@Composable
fun CacheDebugPanel(
    metricsCollector: CacheMetricsCollector,
    staticsCache: StaticsCache,
    memoryPressureMonitor: MemoryPressureMonitor
) {
    val report = metricsCollector.getReport()
    val cacheMetrics = staticsCache.getAllMetrics()
    val memoryInfo by memoryPressureMonitor.memoryInfo.collectAsState()

    Column {
        Text("Memory Pressure: ${memoryInfo.pressureLevel}")
        Text("Used: ${memoryInfo.usedBytes / 1024 / 1024}MB / ${memoryInfo.maxHeapBytes / 1024 / 1024}MB")

        Divider()

        Text("Cache Performance:")
        report.hitRates.forEach { (type, rate) ->
            Text("  $type: ${(rate * 100).toInt()}% hit rate")
        }
        Text("Estimated time saved: ${report.estimatedTimeSavedMs}ms")

        Divider()

        Text("Cache Entries:")
        cacheMetrics.forEach { (type, metrics) ->
            Text("  $type: ${metrics.currentEntries} entries, ${metrics.currentSizeBytes / 1024}KB")
        }
    }
}
```

---

## Phase 5: Testing Strategy

### 5.1 Unit Tests

**File:** `domain/src/test/java/com/lelloman/pezzottify/android/domain/cache/LruCacheTest.kt`

```kotlin
class LruCacheTest {

    @Test
    fun `cache returns null for missing key`() {
        val cache = createTestCache()
        assertThat(cache.get("missing")).isNull()
    }

    @Test
    fun `cache returns stored value`() {
        val cache = createTestCache()
        cache.put("key1", "value1")
        assertThat(cache.get("key1")).isEqualTo("value1")
    }

    @Test
    fun `cache evicts LRU entry when max entries exceeded`() {
        val cache = createTestCache(maxEntries = 2)
        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.get("key1") // Access key1 to make key2 LRU
        cache.put("key3", "value3") // Should evict key2

        assertThat(cache.get("key1")).isEqualTo("value1")
        assertThat(cache.get("key2")).isNull()
        assertThat(cache.get("key3")).isEqualTo("value3")
    }

    @Test
    fun `cache expires entries after TTL`() {
        var currentTime = 0L
        val cache = LruCache<String, String>(
            maxEntries = { 100 },
            maxSizeBytes = { 1_000_000 },
            ttlMillis = 1000,
            sizeCalculator = { 10 },
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        assertThat(cache.get("key1")).isEqualTo("value1")

        currentTime = 1001 // Advance past TTL
        assertThat(cache.get("key1")).isNull()
    }

    @Test
    fun `cache evicts when size limit exceeded`() {
        val cache = LruCache<String, String>(
            maxEntries = { 100 },
            maxSizeBytes = { 100 }, // 100 bytes max
            ttlMillis = 60_000,
            sizeCalculator = { 50 } // Each entry is 50 bytes
        )

        cache.put("key1", "value1") // 50 bytes
        cache.put("key2", "value2") // Would be 100 bytes total
        cache.put("key3", "value3") // Should evict key1

        assertThat(cache.get("key1")).isNull()
    }

    @Test
    fun `metrics track hits and misses correctly`() {
        val cache = createTestCache()
        cache.put("key1", "value1")

        cache.get("key1") // hit
        cache.get("key1") // hit
        cache.get("missing") // miss

        val metrics = cache.getMetrics()
        assertThat(metrics.hits).isEqualTo(2)
        assertThat(metrics.misses).isEqualTo(1)
        assertThat(metrics.hitRate).isWithin(0.01).of(0.67)
    }

    private fun createTestCache(maxEntries: Int = 100) = LruCache<String, String>(
        maxEntries = { maxEntries },
        maxSizeBytes = { 1_000_000 },
        ttlMillis = 60_000,
        sizeCalculator = { 10 }
    )
}
```

### 5.2 Memory Pressure Tests

**File:** `app/src/test/java/com/lelloman/pezzottify/android/memory/AndroidMemoryPressureMonitorTest.kt`

```kotlin
class AndroidMemoryPressureMonitorTest {

    @Test
    fun `returns correct pressure level based on memory usage`() {
        // Test with mocked Runtime values
    }

    @Test
    fun `recommended cache size decreases with higher pressure`() {
        // Verify cache size recommendations scale with pressure
    }

    @Test
    fun `onTrimMemory triggers refresh`() {
        // Verify ComponentCallbacks2 integration
    }
}
```

### 5.3 Integration Tests

**File:** `domain/src/test/java/com/lelloman/pezzottify/android/domain/statics/StaticsProviderCacheTest.kt`

```kotlin
class StaticsProviderCacheTest {

    @Test
    fun `second request for same artist returns cached value`() = runTest {
        // Setup
        val provider = createProviderWithCache()

        // First request - cache miss, loads from DB
        provider.provideArtist("artist-1").first()

        // Second request - should return immediately from cache
        val start = System.nanoTime()
        provider.provideArtist("artist-1").first()
        val elapsed = System.nanoTime() - start

        // Cache hit should be <1ms (vs ~5-10ms for DB)
        assertThat(elapsed).isLessThan(1_000_000) // 1ms in nanos
    }

    @Test
    fun `cache is cleared on logout`() {
        // Verify cache clears properly
    }
}
```

---

## Phase 6: Implementation Checklist

### Step 1: Memory Pressure Component
- [ ] Create `MemoryPressureMonitor` interface in domain
- [ ] Create `CacheItemType` enum in domain
- [ ] Create `MemoryInfo` data class in domain
- [ ] Implement `AndroidMemoryPressureMonitor` in app module
- [ ] Add Hilt module for DI
- [ ] Write unit tests for memory pressure logic
- [ ] Test on low-memory emulator configuration

### Step 2: Cache Infrastructure
- [ ] Create `CacheEntry` data class
- [ ] Create `LruCache` generic implementation
- [ ] Create `CacheMetrics` data class
- [ ] Write comprehensive unit tests for LruCache
- [ ] Test TTL expiration
- [ ] Test LRU eviction
- [ ] Test size-based eviction

### Step 3: StaticsCache
- [ ] Create `StaticsCache` class with typed caches
- [ ] Configure appropriate TTL (start with 5 minutes)
- [ ] Configure size estimates per type
- [ ] Add `clearAll()` method
- [ ] Add `getAllMetrics()` method

### Step 4: StaticsProvider Integration
- [ ] Inject `StaticsCache` into `StaticsProvider`
- [ ] Inject `AppSettings` into `StaticsProvider`
- [ ] Add cache check before DB query in `provideArtist()`
- [ ] Add cache check before DB query in `provideAlbum()`
- [ ] Add cache check before DB query in `provideTrack()`
- [ ] Respect `isInMemoryCacheEnabled` setting in all cache operations
- [ ] Populate cache on successful DB loads (only when enabled)
- [ ] Add `clearCache()` method
- [ ] Call `clearCache()` on logout

### Step 5: User Settings
- [ ] Create `AppSettings` interface in domain
- [ ] Implement `AppSettingsImpl` in localdata with SharedPreferences
- [ ] Add Hilt module for `AppSettings`
- [ ] Create `CacheSettingsSection` composable
- [ ] Integrate settings into ProfileScreen
- [ ] Clear cache when user disables the setting
- [ ] Write unit tests for settings persistence

### Step 6: Metrics
- [ ] Create `CacheMetricsCollector` interface
- [ ] Create `CachePerformanceReport` data class
- [ ] Implement `CacheMetricsCollectorImpl`
- [ ] Inject into `StaticsProvider`
- [ ] Record hits/misses
- [ ] Add latency tracking (optional, may add overhead)

### Step 7: Debug Interface
- [ ] Create `CacheDebugPanel` composable
- [ ] Display memory pressure level
- [ ] Display cache hit rates
- [ ] Display cache sizes
- [ ] Add button to clear cache
- [ ] Add button to reset metrics

### Step 8: Testing & Validation
- [ ] Run unit tests
- [ ] Run on low-end device/emulator
- [ ] Run on high-end device
- [ ] Compare latency: cache hit vs DB query
- [ ] Verify memory usage stays within bounds
- [ ] Verify cache clears on logout
- [ ] Verify cache toggle setting works correctly
- [ ] Verify setting persists across app restarts
- [ ] Verify no memory leaks with LeakCanary

---

## Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `TTL_MILLIS` | 300,000 (5 min) | Cache entry expiration time |
| `LOW_PRESSURE_ENTRIES` | 200/300/500 | Max entries for artist/album/track at low pressure |
| `CRITICAL_PRESSURE_ENTRIES` | 20/30/50 | Max entries at critical pressure |
| `LOW_PRESSURE_BYTES` | 10MB | Total cache size at low pressure |
| `CRITICAL_PRESSURE_BYTES` | 512KB | Total cache size at critical pressure |
| `ARTIST_SIZE_ESTIMATE` | 512 bytes | Estimated size per artist |
| `ALBUM_SIZE_ESTIMATE` | 1024 bytes | Estimated size per album |
| `TRACK_SIZE_ESTIMATE` | 768 bytes | Estimated size per track |

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Stale data | TTL expiration ensures data refreshes periodically |
| Memory pressure on low-end devices | Dynamic sizing based on MemoryPressureMonitor |
| Cache inconsistency after DB updates | Clear cache entries when DB is updated, or invalidate by ID |
| Thread safety | Use ConcurrentHashMap + ReadWriteLock |
| Memory leaks | Clear cache on logout, use weak references if needed |
| Overhead of cache check | Negligible for HashMap lookup (~nanoseconds) |

---

## Future Enhancements

1. **Image caching** - Separate cache for decoded bitmaps with Glide/Coil integration
2. **Audio preloading** - Pre-cache next tracks in queue
3. **Predictive caching** - Pre-fetch related artists/albums based on navigation patterns
4. **Persistent cache** - Optional disk-backed cache for offline scenarios
5. **Cache warming** - Pre-populate cache on app startup with recently viewed items
