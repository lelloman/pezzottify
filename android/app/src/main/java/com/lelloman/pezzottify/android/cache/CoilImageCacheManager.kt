package com.lelloman.pezzottify.android.cache

import com.lelloman.pezzottify.android.domain.cache.ImageCacheManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * [ImageCacheManager] implementation that delegates to a [TrackingDiskCache].
 *
 * The [TrackingDiskCache] tracks cache keys as they are written, enabling
 * age-based trimming without relying on Coil's internal implementation details.
 */
@Singleton
class CoilImageCacheManager @Inject constructor(
    private val trackingDiskCache: TrackingDiskCache,
) : ImageCacheManager {

    override suspend fun getDiskCacheSizeBytes(): Long = withContext(Dispatchers.IO) {
        trackingDiskCache.size
    }

    override suspend fun clear() {
        withContext(Dispatchers.IO) {
            trackingDiskCache.clear()
        }
    }

    override suspend fun trimOldestPercent(percent: Float): Long = withContext(Dispatchers.IO) {
        trackingDiskCache.trimOldestPercent(percent)
    }

    override suspend fun getEntryCount(): Int = withContext(Dispatchers.IO) {
        trackingDiskCache.getEntryCount()
    }
}
