package com.lelloman.pezzottify.android.domain.cache

import com.lelloman.pezzottify.android.domain.memory.CacheItemType
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class StaticsCache @Inject constructor(
    private val memoryPressureMonitor: MemoryPressureMonitor,
    loggerFactory: LoggerFactory,
) {
    private val logger: Logger by loggerFactory

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
        logger.info("clearAll() clearing all caches")
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
        logger.debug("resetAllMetrics() resetting metrics for all caches")
        artistCache.resetMetrics()
        albumCache.resetMetrics()
        trackCache.resetMetrics()
    }

    /**
     * Returns the total size of all caches in bytes.
     */
    fun getTotalSizeBytes(): Long {
        return artistCache.getSizeBytes() +
            albumCache.getSizeBytes() +
            trackCache.getSizeBytes()
    }

    /**
     * Returns the total number of entries across all caches.
     */
    fun getTotalEntryCount(): Int {
        return artistCache.getEntryCount() +
            albumCache.getEntryCount() +
            trackCache.getEntryCount()
    }

    /**
     * Trims all caches by removing the oldest entries by creation time.
     * @param percent The percentage of entries to remove (0.0 to 1.0)
     * @return The total number of entries removed across all caches
     */
    fun trimOldestPercent(percent: Float): Int {
        logger.info("trimOldestPercent($percent) trimming caches")
        return artistCache.trimOldestPercent(percent) +
            albumCache.trimOldestPercent(percent) +
            trackCache.trimOldestPercent(percent)
    }
}
