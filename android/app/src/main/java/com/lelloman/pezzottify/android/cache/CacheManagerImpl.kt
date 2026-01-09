package com.lelloman.pezzottify.android.cache

import com.lelloman.pezzottify.android.domain.cache.CacheManager
import com.lelloman.pezzottify.android.domain.cache.CacheStats
import com.lelloman.pezzottify.android.domain.cache.ImageCacheManager
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class CacheManagerImpl @Inject constructor(
    private val staticsCache: StaticsCache,
    private val staticsStore: StaticsStore,
    private val imageCacheManager: ImageCacheManager,
) : CacheManager {

    override suspend fun getStats(): CacheStats = withContext(Dispatchers.IO) {
        CacheStats(
            staticsDatabaseSizeBytes = staticsStore.getDatabaseSizeBytes(),
            staticsMemorySizeBytes = staticsCache.getTotalSizeBytes(),
            imageCacheSizeBytes = imageCacheManager.getDiskCacheSizeBytes(),
        )
    }

    override suspend fun trimStaticsCache() {
        withContext(Dispatchers.IO) {
            // Trim in-memory cache (50%)
            staticsCache.trimOldestPercent(0.5f)
            // Trim database cache (50%)
            staticsStore.trimOldestPercent(0.5f)
        }
    }

    override suspend fun clearStaticsCache() {
        withContext(Dispatchers.IO) {
            staticsCache.clearAll()
            staticsStore.deleteAll()
            // Vacuum to reclaim disk space - SQLite doesn't shrink file after DELETE
            staticsStore.vacuum()
        }
    }

    override suspend fun trimImageCache() {
        imageCacheManager.trimOldestPercent(0.5f)
    }

    override suspend fun clearImageCache() {
        imageCacheManager.clear()
    }
}
