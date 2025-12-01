package com.lelloman.pezzottify.android.domain.cache

import com.lelloman.pezzottify.android.domain.memory.CacheItemType
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
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

    val artistCache: LruCache<String, Artist> = LruCache(
        maxEntries = { memoryPressureMonitor.getRecommendedMaxEntries(CacheItemType.ARTIST) },
        maxSizeBytes = { memoryPressureMonitor.getRecommendedCacheSizeBytes() / 3 },
        ttlMillis = TTL_MILLIS,
        sizeCalculator = { ARTIST_SIZE_BYTES }
    )

    val albumCache: LruCache<String, Album> = LruCache(
        maxEntries = { memoryPressureMonitor.getRecommendedMaxEntries(CacheItemType.ALBUM) },
        maxSizeBytes = { memoryPressureMonitor.getRecommendedCacheSizeBytes() / 3 },
        ttlMillis = TTL_MILLIS,
        sizeCalculator = { ALBUM_SIZE_BYTES }
    )

    val trackCache: LruCache<String, Track> = LruCache(
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

    fun resetAllMetrics() {
        artistCache.resetMetrics()
        albumCache.resetMetrics()
        trackCache.resetMetrics()
    }
}
