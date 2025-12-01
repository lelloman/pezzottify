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

enum class CacheItemType {
    ARTIST,
    ALBUM,
    TRACK
}

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
