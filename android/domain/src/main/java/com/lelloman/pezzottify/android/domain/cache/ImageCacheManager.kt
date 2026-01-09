package com.lelloman.pezzottify.android.domain.cache

/**
 * Interface for managing the image disk cache.
 */
interface ImageCacheManager {
    /**
     * Returns the current disk cache size in bytes.
     */
    suspend fun getDiskCacheSizeBytes(): Long

    /**
     * Clears all cached images.
     */
    suspend fun clear()

    /**
     * Trims the cache by removing the oldest entries (approximately 50%).
     * @param percent The percentage of entries to remove (0.0 to 1.0)
     * @return Approximate bytes freed
     */
    suspend fun trimOldestPercent(percent: Float): Long

    /**
     * Returns the approximate number of cached entries.
     */
    suspend fun getEntryCount(): Int
}
