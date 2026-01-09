package com.lelloman.pezzottify.android.domain.cache

/**
 * Aggregate cache statistics.
 */
data class CacheStats(
    val staticsDatabaseSizeBytes: Long,
    val staticsMemorySizeBytes: Long,
    val imageCacheSizeBytes: Long,
) {
    val totalStaticsCacheSizeBytes: Long
        get() = staticsDatabaseSizeBytes + staticsMemorySizeBytes
}

/**
 * Manages all application caches (memory, database, images).
 */
interface CacheManager {
    /**
     * Returns current statistics for all caches.
     */
    suspend fun getStats(): CacheStats

    /**
     * Trims the statics cache (both in-memory and database) by removing oldest 50% of entries.
     */
    suspend fun trimStaticsCache()

    /**
     * Clears all statics cache data (both in-memory and database).
     */
    suspend fun clearStaticsCache()

    /**
     * Trims the image cache by removing oldest 50% of entries.
     */
    suspend fun trimImageCache()

    /**
     * Clears all cached images.
     */
    suspend fun clearImageCache()
}
