package com.lelloman.pezzottify.android.domain.storage

import kotlinx.coroutines.flow.StateFlow

enum class StoragePressureLevel {
    LOW,      // Plenty of storage available (>20%)
    MEDIUM,   // Moderate storage available (10-20%)
    HIGH,     // Low storage, be cautious (5-10%)
    CRITICAL  // Very low storage, avoid new downloads (<5%)
}

data class StorageInfo(
    val totalBytes: Long,
    val availableBytes: Long,
    val usedBytes: Long,
    val pressureLevel: StoragePressureLevel
) {
    val usedPercentage: Double
        get() = if (totalBytes > 0) {
            usedBytes.toDouble() / totalBytes.toDouble()
        } else {
            0.0
        }

    val availablePercentage: Double
        get() = if (totalBytes > 0) {
            availableBytes.toDouble() / totalBytes.toDouble()
        } else {
            0.0
        }
}

interface StorageMonitor {
    /**
     * Current storage info as a StateFlow for reactive updates
     */
    val storageInfo: StateFlow<StorageInfo>

    /**
     * Check if we can safely allocate the given number of bytes
     * @param sizeBytes The size to allocate
     * @param keepBuffer If true, maintains a safety buffer (default: true)
     * @return true if allocation is safe
     */
    fun canAllocate(sizeBytes: Long, keepBuffer: Boolean = true): Boolean

    /**
     * Get the recommended maximum size for persistent cache based on storage pressure
     */
    fun getRecommendedMaxCacheBytes(): Long

    /**
     * Force a storage info refresh
     */
    fun refresh()
}
